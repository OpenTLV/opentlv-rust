# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Publish the attested crate pair of a GitHub release to crates.io after approval."""

import argparse
import hashlib
import json
import os
import shutil
import time
import urllib.error
import urllib.request

from ci import Cargo, install
from ci_contract import CRATES, REPOSITORY, definition_hashes, lock, matrix
from ci_support import ROOT, api, digest, read, require, run, write
import release

WORKFLOW = '.github/workflows/rust-crates-io.yml'
SIGNER = REPOSITORY + '/' + WORKFLOW
ENVIRONMENT = 'crates-io'
# The account that claims both names at the first publication; it stays an owner.
OWNER = 'marekcingel'
CRATES_API = 'https://crates.io/api/v1/crates'
INDEX = 'https://index.crates.io'
DOWNLOADS = 'https://static.crates.io/crates'
# The crates.io crawler policy requires a User-Agent that identifies the client.
USER_AGENT = f'opentlv-rust-release (https://github.com/{REPOSITORY})'
WORK = ROOT / 'build/crates-io'
WAIT_SECONDS = 600
POLL_SECONDS = 15


def fetch(url, missing=(404,)):
    request = urllib.request.Request(url, headers={'User-Agent': USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if error.code in missing:
            return None
        raise


def owners(crate):
    """The owners of a crate, or None for a free name; crates.io treats - and _ as one name."""
    data = fetch(f'{CRATES_API}/{crate}/owners')
    return None if data is None else {item['login'] for item in json.loads(data)['users']}


def index_entry(crate, version):
    data = fetch(f'{INDEX}/{crate[:2]}/{crate[2:4]}/{crate}')
    entries = [] if data is None else [json.loads(line) for line in data.splitlines()]
    return next((entry for entry in entries if entry['vers'] == version), None)


def upload_plan(version, packages):
    """The crates still to upload, dependency first, and the credential they need."""
    uploads = []
    first_version = False
    for crate in CRATES:
        logins = owners(crate)
        first_version |= logins is None
        require(logins is None or OWNER in logins, f'{crate} on crates.io is not owned by {OWNER}')
        entry = index_entry(crate, version)
        if entry is None:
            uploads.append(crate)
        else:
            # A published version is immutable: it can only be confirmed, never replaced.
            require(entry['cksum'] == packages[crate] and not entry['yanked'],
                    f'{crate} {version} is already on crates.io with other bytes or yanked; '
                    'release a new version')
    # crates.io accepts Trusted Publishing only for a crate that already exists.
    auth = 'none' if not uploads else 'bootstrap' if first_version else 'trusted'
    return uploads, auth


def protected_environment():
    """Referencing a missing environment would create it unprotected, so check first."""
    path = f'repos/{REPOSITORY}/environments/{ENVIRONMENT}'
    environment = api(path)
    rules = {rule['type']: rule for rule in environment.get('protection_rules', [])}
    require(rules.get('required_reviewers', {}).get('reviewers'),
            f'The {ENVIRONMENT} environment must require a reviewer')
    require(environment.get('can_admins_bypass') is False,
            f'The {ENVIRONMENT} environment must not let administrators bypass its rules')
    require((environment.get('deployment_branch_policy') or {}).get('custom_branch_policies') is True,
            f'The {ENVIRONMENT} environment must be limited to main')
    branches = api(path + '/deployment-branch-policies')['branch_policies']
    require([(item['name'], item.get('type')) for item in branches] == [('main', 'branch')],
            f'The {ENVIRONMENT} environment must be limited to main')


def approved_by(run_id):
    history = api(f'repos/{REPOSITORY}/actions/runs/{run_id}/approvals')
    reviewers = sorted({item['user']['login'] for item in history if item['state'] == 'approved'
                        and any(place['name'] == ENVIRONMENT for place in item['environments'])})
    require(reviewers, 'This run has no recorded approval of the crates.io publication')
    return reviewers


def verified_release(tag, commit, assets):
    """Download a public release and prove that it is the attested, tested state of this commit."""
    version = release.version_for(tag)
    # Scripts, CI policy and packaged sources are the release's only while main stays at its commit.
    require(api(f'repos/{REPOSITORY}/git/ref/heads/main')['object']['sha'] == commit,
            'Main moved; only the current main commit can be published')
    require(api(f'repos/{REPOSITORY}/commits/{tag}')['sha'] == commit,
            'The release tag is not at the current main commit')
    matches = [item for item in release.pages(f'repos/{REPOSITORY}/releases') if item['tag_name'] == tag]
    require(len(matches) == 1, 'Missing or duplicate GitHub release')
    published = api(f'repos/{REPOSITORY}/releases/{matches[0]["id"]}')
    run('gh', 'release', 'download', tag, '--repo', REPOSITORY, '--dir', assets)
    release.verify_attestations(assets, commit)
    request = release.verify_contents(assets)
    require(request['tag'] == tag and request['version'] == version and
            request['identity']['commit'] == commit and
            request['definitions'] == definition_hashes() and request['core'] == lock(True) and
            request['notes_sha256'] == digest(release.NOTES),
            'Release evidence differs from this checkout')
    release.verify_release_files(published, assets, request, False)
    release.main_evidence(commit)
    return request, published


def check_packaged(cargo, crates, version, packages):
    """Cargo cannot upload an existing archive, so prove that it packs the tested bytes."""
    selection = (['--workspace'] if set(crates) == set(CRATES)
                 else [argument for crate in crates for argument in ('-p', crate)])
    # Without verification, no build script runs near a publishing credential.
    cargo('package', *selection, '--locked', '--no-verify')
    for crate in crates:
        require(digest(WORK / 'target/package' / f'{crate}-{version}.crate') == packages[crate],
                f'Cargo packs other bytes than the tested {crate} archive')


def publisher():
    """The configuration whose toolchain built the tested archives."""
    configuration = next(item for item in matrix('full') if item['id'] == 'linux-stable')
    install(configuration)
    return Cargo(configuration, WORK, False)


def prepare(tag, approved):
    current = release.hosted_identity(SIGNER)
    shutil.rmtree(WORK, ignore_errors=True)
    WORK.mkdir(parents=True)
    protected_environment()
    request, published = verified_release(tag, current['commit'], WORK / 'assets')
    version = request['version']
    packages = {crate: digest(WORK / 'assets' / f'{crate}-{version}.crate') for crate in CRATES}
    uploads, auth = upload_plan(version, packages)
    if uploads:
        check_packaged(publisher(), uploads, version, packages)
    plan = {'schema': 1, 'tag': tag, 'version': version, 'release': published['html_url'],
            'release_identity': request['identity'], 'publisher_identity': current,
            'packages': packages, 'uploads': uploads, 'auth': auth,
            'approved_by': approved_by(current['run_id']) if approved else None}
    write(WORK / 'plan.json', plan)
    print(json.dumps(plan, indent=2))
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
        output.write(f'auth={auth}\n')


def published(crate, version, expected):
    """Whether the index and the download serve the expected bytes; other bytes are a failure."""
    entry = index_entry(crate, version)
    if entry is None:
        return False
    require(entry['cksum'] == expected and not entry['yanked'], f'crates.io lists other {crate} bytes')
    # The download host answers 403 for a file it does not have yet.
    data = fetch(f'{DOWNLOADS}/{crate}/{crate}-{version}.crate', missing=(403, 404))
    if data is None:
        return False
    require(hashlib.sha256(data).hexdigest() == expected, f'crates.io serves other {crate} bytes')
    return True


def wait_published(crate, version, expected):
    deadline = time.monotonic() + WAIT_SECONDS
    while not published(crate, version, expected):
        require(time.monotonic() < deadline, f'{crate} {version} did not appear on crates.io')
        time.sleep(POLL_SECONDS)


def upload(plan, token):
    require(token, 'Missing crates.io credential')
    cargo = publisher()
    version = plan['version']
    for crate in plan['uploads']:
        # The safe crate resolves its dependency from the registry only after that upload.
        check_packaged(cargo, [crate], version, plan['packages'])
        cargo('publish', '-p', crate, '--locked', '--no-verify', env={'CARGO_REGISTRY_TOKEN': token})
        wait_published(crate, version, plan['packages'][crate])


def confirm(plan):
    for crate in CRATES:
        wait_published(crate, plan['version'], plan['packages'][crate])
    write(WORK / 'receipt.json', {**plan, 'status': 'success', 'crates': {
        crate: f'https://crates.io/crates/{crate}/{plan["version"]}' for crate in CRATES}})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('prepare', 'upload', 'confirm'))
    parser.add_argument('--approved', action='store_true',
                        help='Running in the protected job: require a recorded approval')
    args = parser.parse_args()
    # Cargo's environment is built from os.environ; only the publishing call receives this.
    token = os.environ.pop('CARGO_REGISTRY_TOKEN', '')
    if args.command == 'prepare':
        prepare(os.environ['RELEASE_TAG'], args.approved)
    elif args.command == 'upload':
        upload(read(WORK / 'plan.json'), token)
    else:
        confirm(read(WORK / 'plan.json'))


if __name__ == '__main__':
    main()
