# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Publish the tested Rust crate pair from a clean main dispatch and Full attempt."""

import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import tomllib

from ci_contract import (CRATES, REPOSITORY, definition_hashes, identity, lock,
                         validate_records)
from ci_support import ROOT, api, digest, read, require, run, write
from ci_timings import fetch_jobs

WORKFLOW = '.github/workflows/rust-release.yml'
SIGNER = REPOSITORY + '/' + WORKFLOW
NOTES = ROOT / 'ci/release-notes.md'
RESUMED = ROOT / 'build/resumed'
# The release list can lag behind the creation of a draft (v0.2.1 failed on that).
DRAFT_WAIT_SECONDS = 300
DRAFT_POLL_SECONDS = 10


def pages(endpoint, key=None):
    result = []
    for page in range(1, 101):
        separator = '&' if '?' in endpoint else '?'
        data = api(endpoint + f'{separator}per_page=100&page={page}')
        items = data[key] if key else data
        result.extend(items)
        if len(items) < 100:
            return result
    raise ValueError('GitHub pagination limit exceeded')


def version_for(tag):
    require(re.fullmatch(r'v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', tag),
            'Use a stable vMAJOR.MINOR.PATCH tag')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))[
        'workspace']['package']['version']
    require(tag == 'v' + version, 'Tag differs from Cargo version')
    locked = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))['package']
    require({p['name']: p['version'] for p in locked} == dict.fromkeys(CRATES, version),
            'Cargo.lock versions differ')
    dependency = tomllib.loads((ROOT / 'opentlv/Cargo.toml').read_text(encoding='utf-8'))[
        'dependencies']['opentlv-sys']
    require(dependency['version'] == '=' + version, 'Crate pair version differs')
    notes = NOTES.read_text(encoding='utf-8')
    require(notes.startswith(f'# OpenTLV Rust {tag}\n') and
            re.search(r'^- \S', notes, re.MULTILINE), 'Missing versioned release notes')
    return version


def hosted_identity(workflow=SIGNER):
    current = identity()
    require(current['hosted'] and not current['dirty'] and current['repository'] == REPOSITORY,
            'Release requires the official clean hosted checkout')
    require(os.environ.get('GITHUB_EVENT_NAME') == 'workflow_dispatch' and
            os.environ.get('GITHUB_REF') == 'refs/heads/main' and
            os.environ.get('GITHUB_WORKFLOW_REF') == workflow + '@refs/heads/main' and
            os.environ.get('GITHUB_WORKFLOW_SHA') == current['commit'],
            'Release requires a main dispatch with matching workflow SHA')
    return current


def release_state(tag, commit):
    """None for an unused tag, or what an interrupted publication of this commit left behind."""
    require(api(f'repos/{REPOSITORY}/git/ref/heads/main')['object']['sha'] == commit,
            'Main moved; validate and release the new commit')
    refs = api(f'repos/{REPOSITORY}/git/matching-refs/tags/{tag}')
    tagged = any(ref['ref'] == 'refs/tags/' + tag for ref in refs)
    releases = [item for item in pages(f'repos/{REPOSITORY}/releases') if item['tag_name'] == tag]
    if not tagged and not releases:
        return None
    # Only the tag and draft of this commit can be resumed; nothing else is ever changed.
    require(tagged, 'A release exists without its tag')
    require(api(f'repos/{REPOSITORY}/commits/{tag}')['sha'] == commit,
            'Tag already exists at another commit; release a new version')
    require(len(releases) <= 1, 'Duplicate releases for the tag')
    require(not releases or (releases[0]['draft'] is True and releases[0]['prerelease'] is False),
            'Release already exists')
    return {'draft': releases[0]['id'] if releases else None}


def check_main(run_info, commit):
    require(run_info.get('head_sha') == commit and run_info.get('event') == 'push' and
            run_info.get('head_branch') == 'main' and
            run_info.get('path') == '.github/workflows/rust-main.yml' and
            run_info.get('repository', {}).get('full_name') == REPOSITORY and
            run_info.get('head_repository', {}).get('full_name') == REPOSITORY and
            run_info.get('status') == 'completed' and run_info.get('conclusion') == 'success',
            'Successful Main CI on the release commit is required')


def main_evidence(commit):
    runs = pages(f'repos/{REPOSITORY}/actions/workflows/rust-main.yml/runs'
                 f'?head_sha={commit}&event=push&branch=main', 'workflow_runs')
    require(runs, 'Main CI is missing on the release commit')
    latest = max(runs, key=lambda item: item['id'])
    check_main(latest, commit)
    return {'run_id': latest['id'], 'run_attempt': latest['run_attempt'],
            'url': latest['html_url']}


def check_gate(run_id, attempt):
    jobs = fetch_jobs(REPOSITORY, run_id, attempt)
    gates = [j for j in jobs if j['name'].endswith(' / Rust quality gate')]
    require(len(gates) == 1 and gates[0].get('status') == 'completed' and
            gates[0].get('conclusion') == 'success', 'Missing successful attempt-bound quality gate')


def preflight(tag):
    version = version_for(tag)
    current = hosted_identity()
    resume = release_state(tag, current['commit'])
    main = main_evidence(current['commit'])
    check_gate(main['run_id'], main['run_attempt'])
    core = lock(True)
    require(core['release_tag'] == 'v' + core['version'], 'Core tag/version differs')
    return {'schema': 1, 'tag': tag, 'version': version, 'identity': current,
            'definitions': definition_hashes(), 'core': core, 'main': main,
            'notes_sha256': digest(NOTES), 'resume': resume}


def validate_request(request):
    require(request == preflight(request['tag']), 'Release request, Main evidence or source changed')


def check_full(quality, records, request):
    computed = validate_records(records, 'full', request['identity'], request['definitions'],
                                quality.get('checks', {}), True)
    require(quality == computed, 'Full gate differs from complete build evidence')
    current = request['identity']
    check_gate(current['run_id'], current['run_attempt'])


def assemble(evidence, output):
    request = read(evidence / 'request.json')
    validate_request(request)
    quality = read(evidence / 'quality.json')
    records = [read(p) for p in sorted((evidence / 'builds').rglob('record.json'))]
    check_full(quality, records, request)
    linux = next(r for r in records if r['configuration']['id'] == 'linux-stable')
    packages = {f'{crate}-{request["version"]}.crate': linux['package'][crate] for crate in CRATES}
    require({p.name for p in (evidence / 'packages').iterdir()} == set(packages),
            'Unexpected crate package set')
    for name, expected in packages.items():
        require(digest(evidence / 'packages' / name) == expected, 'Crate differs from tested package')
    output.mkdir(parents=True, exist_ok=False)
    for name in packages:
        shutil.copyfile(evidence / 'packages' / name, output / name)
    shutil.copyfile(NOTES, output / 'release-notes.md')
    write(output / 'release-manifest.json', {'schema': 1, 'request': request, 'quality': quality,
                                          'records': records, 'packages': packages})
    (output / 'SHA256SUMS').write_text(''.join(
        f'{digest(p)}  {p.name}\n' for p in sorted(output.iterdir())), encoding='utf-8')


def verify_payload(directory):
    validate_request(read(directory / 'release-manifest.json')['request'])
    return verify_contents(directory)


def verify_contents(directory):
    manifest = read(directory / 'release-manifest.json')
    require(manifest.get('schema') == 1, 'Unsupported release manifest')
    request = manifest['request']
    check_full(manifest['quality'], manifest['records'], request)
    linux = next(r for r in manifest['records'] if r['configuration']['id'] == 'linux-stable')
    packages = {f'{c}-{request["version"]}.crate': linux['package'][c] for c in CRATES}
    require(manifest['packages'] == packages, 'Release package identity differs')
    names = set(packages) | {'release-manifest.json', 'release-notes.md'}
    require({p.name for p in directory.iterdir()} == names | {'SHA256SUMS'},
            'Unexpected release asset set')
    expected_sums = ''.join(f'{digest(directory / name)}  {name}\n' for name in sorted(names))
    require((directory / 'SHA256SUMS').read_text(encoding='utf-8') == expected_sums,
            'Release checksums differ')
    require(digest(directory / 'release-notes.md') == request['notes_sha256'], 'Release notes changed')
    for name, expected in packages.items():
        require(digest(directory / name) == expected, 'Crate differs from tested package')
    return request


def verify_published_assets(release, directory, request, draft):
    require(release.get('body', '').strip() == NOTES.read_text(encoding='utf-8').strip(),
            'Published release metadata differs')
    verify_release_files(release, directory, request, draft)


def verify_release_files(release, directory, request, draft):
    """The release state and assets; a maintainer can still edit the text of a public release."""
    require(release.get('tag_name') == request['tag'] and release.get('draft') is draft and
            release.get('prerelease') is False, 'Published release metadata differs')
    assets = release.get('assets', [])
    paths = {p.name: p for p in directory.iterdir()}
    require(len(assets) == len(paths) and {a['name'] for a in assets} == set(paths),
            'Published release asset set differs')
    for asset in assets:
        path = paths[asset['name']]
        require(asset.get('state') == 'uploaded' and asset.get('size') == path.stat().st_size and
                asset.get('digest') == 'sha256:' + digest(path), 'Published asset digest differs')


def verify_attestations(directory, commit):
    for path in sorted(directory.iterdir()):
        run('gh', 'attestation', 'verify', path, '--repo', REPOSITORY,
            '--signer-workflow', SIGNER, '--source-digest', commit)


def find_draft(tag):
    """The draft just created, the only release of its tag, waiting while the release list lags."""
    deadline = time.monotonic() + DRAFT_WAIT_SECONDS
    while True:
        matches = [item for item in pages(f'repos/{REPOSITORY}/releases') if item['tag_name'] == tag]
        require(len(matches) <= 1, 'Duplicate releases for the tag')
        if matches:
            require(matches[0]['draft'] is True, 'The new release is not a draft')
            return api(f'repos/{REPOSITORY}/releases/{matches[0]["id"]}')
        require(time.monotonic() < deadline, 'The new draft release did not appear')
        time.sleep(DRAFT_POLL_SECONDS)


def download(asset, path):
    command = ['gh', 'api', '-H', 'Accept: application/octet-stream',
               f'repos/{REPOSITORY}/releases/assets/{asset["id"]}']
    print('+ ' + ' '.join(command), flush=True)
    with open(path, 'wb') as output:
        subprocess.run(command, cwd=ROOT, check=True, stdout=output)


def resumed_draft(release, directory, request):
    """Prove that an interrupted publication's draft is a verified release of this tested state."""
    shutil.rmtree(RESUMED, ignore_errors=True)
    RESUMED.mkdir(parents=True)
    for asset in release.get('assets', []):
        require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', asset['name']), 'Unexpected release asset name')
        download(asset, RESUMED / asset['name'])
    verify_release_files(release, RESUMED, request, True)
    commit = request['identity']['commit']
    verify_attestations(RESUMED, commit)
    # Its own Full evidence and gate, then the identity of what this run has just tested.
    earlier = verify_contents(RESUMED)
    require(all(earlier[key] == request[key] for key in ('tag', 'version', 'definitions', 'core',
                                                         'notes_sha256')) and
            earlier['identity']['commit'] == commit,
            'The existing draft is not a release of this verified state; record it and remove it '
            'by hand, or release a new version')
    require(read(RESUMED / 'release-manifest.json')['packages'] ==
            read(directory / 'release-manifest.json')['packages'],
            'The existing draft has other crate bytes than this run tested')
    return RESUMED


def publish(directory):
    request = verify_payload(directory)
    commit = request['identity']['commit']
    tag = request['tag']
    verify_attestations(directory, commit)
    # Final read before the first write; never overwrite or retarget an existing tag or release.
    state = release_state(tag, commit)
    require(state == request['resume'], 'Release state changed since the request')
    if state is None:
        run('gh', 'api', '--method', 'POST', f'repos/{REPOSITORY}/git/refs',
            '-f', 'ref=refs/tags/' + tag, '-f', 'sha=' + commit)
    if state is None or state['draft'] is None:
        run('gh', 'release', 'create', tag, '--repo', REPOSITORY, '--verify-tag',
            '--draft', '--title', 'OpenTLV Rust ' + tag,
            '--notes-file', directory / 'release-notes.md', *sorted(directory.iterdir()))
        release = find_draft(tag)
        payload = directory
    else:
        # A draft is never changed: it is published as it is, or the run stops.
        release = api(f'repos/{REPOSITORY}/releases/{state["draft"]}')
        payload = resumed_draft(release, directory, request)
    verify_published_assets(release, payload, request, True)
    require(api(f'repos/{REPOSITORY}/commits/{tag}')['sha'] == commit, 'Release tag changed')
    # By identity: a lookup by tag would depend on the release list again.
    run('gh', 'api', '--method', 'PATCH', f'repos/{REPOSITORY}/releases/{release["id"]}',
        '-F', 'draft=false')
    release = api(f'repos/{REPOSITORY}/releases/{release["id"]}')
    verify_published_assets(release, payload, request, False)
    require(api(f'repos/{REPOSITORY}/commits/{tag}')['sha'] == commit, 'Published tag changed')
    write(ROOT / 'build/published.json', {'url': release['html_url'], 'tag': tag,
                                        'identity': request['identity'], 'core': request['core'],
                                        'resumed': state is not None,
                                        'assets': [a['name'] for a in release['assets']]})
    print(release['html_url'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('prepare', 'assemble', 'publish'))
    args = parser.parse_args()
    if args.command == 'prepare':
        output = ROOT / 'build/request.json'
        output.unlink(missing_ok=True)
        write(output, preflight(os.environ['RELEASE_TAG']))
    elif args.command == 'assemble':
        assemble(ROOT / 'build/evidence', ROOT / 'build/release')
    else:
        (ROOT / 'build/published.json').unlink(missing_ok=True)
        publish(ROOT / 'build/release')


if __name__ == '__main__':
    main()
