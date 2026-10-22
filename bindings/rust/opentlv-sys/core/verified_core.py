# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Authenticate a locked Core release (Core #637 schema-2 assets) for build.rs.

Run by build.rs only when core-artifact.json locks a release. Downloads the
release manifest, checksums and selected archives with the GitHub CLI,
verifies their GitHub Artifact Attestations, and selects a compatible static
Verified Binary or the Verified Source archive. Uses only the standard library.
"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile

REPOSITORY = 'OpenTLV/opentlv'
SIGNER = REPOSITORY + '/.github/workflows/core-release.yml'
# Core schema-2 Full evidence contract, shared with the release producer.
FULL_CHECKS = frozenset(('clang', 'static', 'consumers', 'codeql', 'msvc', 'gcc',
                         'properties', 'query', 'fuzz', 'capabilities', 'abi', 'memcheck'))
CONFIGURATION_SOURCES = frozenset((
    'CMakeLists.txt', 'cmake/compiler.cmake', 'cmake/capability_options.cmake',
    'cmake/format_options.cmake', 'scripts/check_cmake_integration.py',
    'scripts/check_capabilities.py', 'scripts/abi/check.py'))
WORKFLOWS = frozenset((
    'build-clang.yml', 'build-msvc.yml', 'static-analysis.yml', 'cmake-consumers.yml',
    'codeql.yml', 'build-gcc.yml', 'property.yml', 'query-hardening.yml', 'fuzz.yml',
    'capabilities.yml', 'abi.yml', 'valgrind.yml', 'core-full.yml',
    'core-checks.yml', 'core-release.yml'))
# Rust targets with an official static Core binary of the same platform.
TARGETS = {'x86_64-pc-windows-msvc': ('windows', 'Windows'),
           'x86_64-unknown-linux-gnu': ('linux', 'Linux')}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args, capture=False):
    command = list(map(str, args))
    # stdout belongs to Cargo's build-script protocol.
    print('+ ' + ' '.join(command), file=sys.stderr, flush=True)
    try:
        result = subprocess.run(command, check=True, text=True, encoding='utf-8',
                                stdout=subprocess.PIPE if capture else sys.stderr)
    except FileNotFoundError as error:
        raise ValueError(f'{command[0]} is required to verify a Core release') from error
    return result.stdout.strip() if capture else None


def read(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def api(path):
    return json.loads(run('gh', 'api', path, capture=True))


def lock(path):
    value = read(path)
    require(value.get('schema') == 1 and value.get('repository') == REPOSITORY,
            'Unsupported Core artifact lock')
    require(isinstance(value.get('commit'), str) and
            re.fullmatch('[0-9a-f]{40}', value['commit']), 'Expected an exact Core commit')
    require(isinstance(value.get('version'), str) and
            re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', value['version']),
            'Expected an exact Core version')
    require(type(value.get('allow_verified_source')) is bool, 'Explicit fallback policy required')
    tag = value.get('release_tag')
    require(tag is not None, 'A published Core schema-2 release must be locked for verified builds')
    require(tag == 'v' + value['version'], 'Core version/tag mismatch')
    return value


def safe_name(name):
    require(isinstance(name, str) and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', name),
            'Unsafe artifact filename')
    return name


def extract(archive, destination):
    """Extract regular files and contained relative tar links; reject traversal and devices."""
    destination.mkdir(parents=True, exist_ok=False)

    def target(name):
        require('\\' not in name and ':' not in name and '\0' not in name, 'Unsafe archive path')
        path = PurePosixPath(name)
        require(not path.is_absolute() and '..' not in path.parts, 'Archive traversal')
        result = destination.joinpath(*path.parts)
        require(result.resolve().is_relative_to(destination.resolve()), 'Archive escapes destination')
        return result

    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as source:
            for member in source.infolist():
                # ZipInfo normalizes backslashes on Windows; inspect the original.
                path = target(member.orig_filename)
                require((member.external_attr >> 16) & 0o170000 != 0o120000, 'ZIP symlink is unsupported')
                if member.is_dir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    require(not path.exists(), 'Duplicate archive file')
                    path.write_bytes(source.read(member))
    else:
        with tarfile.open(archive) as source:
            links = []
            for member in source:
                path = target(member.name)
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    require(not path.exists(), 'Duplicate archive file')
                    with source.extractfile(member) as stream:
                        path.write_bytes(stream.read())
                    path.chmod(member.mode & 0o777)
                elif member.issym():
                    require('\\' not in member.linkname and ':' not in member.linkname and
                            not PurePosixPath(member.linkname).is_absolute(), 'Unsafe symlink')
                    link = (path.parent / member.linkname).resolve()
                    require(link.is_relative_to(destination.resolve()), 'Symlink traversal')
                    links.append((path, member.linkname))
                else:
                    raise ValueError('Unsupported archive member')
            for path, link in links:
                path.parent.mkdir(parents=True, exist_ok=True)
                require(not path.exists() and not path.is_symlink(), 'Duplicate symlink')
                path.symlink_to(link)


def checksums(path):
    result = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  (.+)', line)
        require(match is not None, 'Malformed SHA256SUMS')
        name = safe_name(match[2])
        require(name not in result, 'Duplicate checksum')
        result[name] = match[1]
    return result


def verify_manifest(manifest, selected):
    require(manifest.get('schema') == 2 and manifest.get('commit') == selected['commit'] and
            manifest.get('version') == selected['release_tag'][1:], 'Wrong Core manifest identity/schema')
    quality = manifest.get('quality', {})
    candidate = manifest.get('candidate', {})
    require(quality.get('schema') == 2 and quality.get('tier') == 'full' and
            quality.get('repository') == REPOSITORY and quality.get('commit') == selected['commit'],
            'Core Full CI identity missing')
    require(candidate.get('version') == selected['version'] and
            candidate.get('commit') == selected['commit'] and candidate.get('tag') == selected['release_tag'] and
            candidate.get('repository') == REPOSITORY and candidate.get('run_id') == quality.get('run_id') and
            candidate.get('run_attempt') == quality.get('run_attempt'), 'Core candidate evidence mismatch')
    require(quality.get('run_id') and quality.get('run_attempt') and
            isinstance(quality.get('checks'), dict) and set(quality['checks']) == FULL_CHECKS and
            all(v == 'success' for v in quality['checks'].values()),
            'Core Full CI did not pass')
    configuration = quality.get('configuration', {})
    require(configuration.get('inputs') == {'full': True, 'release_tag': selected['release_tag']} and
            set(configuration.get('configuration_sources', {})) == CONFIGURATION_SOURCES and
            set(configuration.get('workflows', {})) == WORKFLOWS,
            'Core Full configuration is incomplete')
    for record in configuration['workflows'].values():
        require(isinstance(record.get('definition'), str) and
                hashlib.sha256(record['definition'].encode()).hexdigest() == record.get('sha256'),
                'Core workflow definition/hash mismatch')
    names = set()
    for item in manifest.get('artifacts', []):
        name = safe_name(item.get('filename'))
        require(name not in names, 'Duplicate manifest artifact')
        names.add(name)
        require(item.get('commit') == selected['commit'] and item.get('version') == manifest['version'] and
                re.fullmatch('[0-9a-f]{64}', item.get('sha256', '')), 'Invalid Core artifact record')
    require(names, 'Empty Core manifest')


def verify_configuration(manifest, source):
    """Bind recorded workflow/configuration bytes to the authenticated source."""
    configuration = manifest['quality']['configuration']
    for path, expected in configuration['configuration_sources'].items():
        require(digest(source / path) == expected, 'Core configuration source differs: ' + path)
    for name, record in configuration['workflows'].items():
        path = source / '.github/workflows' / name
        require(digest(path) == record['sha256'] and
                path.read_text(encoding='utf-8') == record['definition'],
                'Core workflow differs from authenticated source: ' + name)


def compatible(record, probe, target, host, crt_static, components):
    """True only for a static binary equivalent to a Verified Source build of this target."""
    if target != host or target not in TARGETS:
        return False
    system, cmake_system = TARGETS[target]
    if (platform.system() != cmake_system or probe.get('CROSS_COMPILING') != 'FALSE' or
            probe.get('SYSTEM') != cmake_system or probe.get('SIZEOF_DATA_PTR') != '8' or
            (probe.get('COMPILER_ARCHITECTURE') or probe.get('PROCESSOR', '')).lower()
            not in ('amd64', 'x86_64', 'x64')):
        return False
    if system == 'windows':
        # Rust's default MSVC runtime is the release DLL runtime.
        runtime = not crt_static and record.get('msvc_runtime') == 'MultiThreadedDLL'
    else:
        # Rust links position-independent executables; never infer PIC from x64.
        runtime = (record.get('position_independent_code') is True and
                   record.get('libc') == list(platform.libc_ver()))
    options = record.get('options', {})
    return (runtime and record.get('id') == f'{system}-x64-static' and
            record.get('architecture') == 'x64' and record.get('os') == system and
            record.get('build_type') == 'Release' and record.get('linkage') == 'static' and
            record.get('compiler', {}).get('CMAKE_C_COMPILER_ID') == probe.get('COMPILER_ID') and
            record.get('compiler', {}).get('CMAKE_C_COMPILER_VERSION') == probe.get('COMPILER_VERSION') and
            all(options.get(name) == value for name, value in components.items()))


def authenticated_release(work, selected):
    work.mkdir(parents=True, exist_ok=False)
    release = api(f"repos/{REPOSITORY}/releases/tags/{selected['release_tag']}")
    require(not release.get('draft') and release.get('tag_name') == selected['release_tag'], 'Core release unavailable')
    resolved = api(f"repos/{REPOSITORY}/commits/{selected['release_tag']}")
    require(resolved.get('sha') == selected['commit'], 'Core tag moved away from locked commit')

    def download(name):
        safe_name(name)
        require(sum(a['name'] == name for a in release['assets']) == 1, 'Missing/duplicate Core release asset: ' + name)
        path = work / name
        run('gh', 'release', 'download', selected['release_tag'], '--repo', REPOSITORY,
            '--pattern', name, '--dir', work)
        run('gh', 'attestation', 'verify', path, '--repo', REPOSITORY,
            '--signer-workflow', SIGNER, '--source-digest', selected['commit'])
        return path

    manifest_path = download('release-manifest.json')
    sums_path = download('SHA256SUMS')
    sums = checksums(sums_path)
    require(sums.get(manifest_path.name) == digest(manifest_path), 'Core manifest checksum mismatch')
    manifest = read(manifest_path)
    verify_manifest(manifest, selected)
    return manifest, sums, download, {'release_id': release['id'], 'manifest_sha256': digest(manifest_path)}


def single(paths, message):
    paths = list(paths)
    require(len(paths) == 1, message)
    return paths[0]


def resolve(lock_path, work, target, host, components, probe=None, crt_static=False):
    """Authenticate the locked release; probe=None selects the Verified Source only."""
    selected = lock(lock_path)
    work.mkdir(parents=True, exist_ok=True)
    # A fresh attempt directory; never reuse files from a failed verification.
    attempt = Path(tempfile.mkdtemp(prefix='release-', dir=work))
    manifest, sums, download, release_identity = authenticated_release(attempt / 'assets', selected)
    sources = [r for r in manifest['artifacts'] if r.get('kind') == 'source']
    require(len(sources) == 1, 'Expected one Verified Source artifact')
    binaries = [] if probe is None else [
        r for r in manifest['artifacts'] if r.get('kind') == 'binary' and
        compatible(r, probe, target, host, crt_static, components)]
    require(len(binaries) <= 1, 'Ambiguous compatible Core binary')
    if binaries:
        record, mode = binaries[0], 'verified-binary'
    else:
        # Test data and headers need the source even when only linking.
        require(probe is None or selected['allow_verified_source'],
                'No compatible binary and Verified Source fallback disabled')
        record, mode = sources[0], 'verified-source'

    def fetch(item, destination):
        archive = download(item['filename'])
        require(digest(archive) == item['sha256'] == sums.get(archive.name), 'Core archive checksum mismatch')
        extract(archive, destination)
        return archive

    fetch(sources[0], attempt / 'source')
    source = attempt / 'source' / ('OpenTLV-' + manifest['version'])
    require((source / 'opentlv-source.txt').read_text(encoding='utf-8') ==
            f"version={manifest['version']}\ncommit={selected['commit']}\n", 'Wrong extracted Core identity')
    verify_configuration(manifest, source)
    location = {'source_dir': str(source.resolve())}
    if mode == 'verified-binary':
        package = attempt / 'binary'
        fetch(record, package)
        library = single([p for name in ('tlv.lib', 'libtlv.a') for p in package.rglob(name)],
                         'Expected one static Core library')
        version_h = single(package.rglob('tlv/version.h'), 'Expected one Core public version header')
        require((version_h.parent / 'config.h').is_file(), 'Core package lacks tlv/config.h')
        location.update(lib_dir=str(library.parent.resolve()),
                        include_dir=str(version_h.parent.parent.resolve()))
    evidence = {'mode': mode, 'verified': True, 'version': selected['version'],
                'repository': REPOSITORY, 'commit': selected['commit'], **location,
                'release_tag': selected['release_tag'], **release_identity,
                'signer_workflow': SIGNER, 'artifact': record, 'probe': probe,
                'target': target, 'components': components}
    write(attempt / 'verification.json', evidence)
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--host', required=True)
    parser.add_argument('--components', required=True,
                        help='Comma-separated OPENTLV_NAME=ON|OFF Core options')
    parser.add_argument('--probe', type=Path, help='C toolchain probe; omit with --source-only')
    parser.add_argument('--source-only', action='store_true')
    parser.add_argument('--crt-static', action='store_true')
    args = parser.parse_args()
    # A failed verification must not leave an apparently valid previous result.
    args.output.unlink(missing_ok=True)
    try:
        require(args.source_only != (args.probe is not None), 'Pass exactly one of --probe or --source-only')
        components = dict(item.split('=', 1) for item in args.components.split(','))
        require(all(re.fullmatch('OPENTLV_[A-Z0-9_]+', k) and v in ('ON', 'OFF')
                    for k, v in components.items()), 'Invalid Core components')
        evidence = resolve(args.lock, args.work, args.target, args.host, components,
                           None if args.source_only else read(args.probe), args.crt_static)
        keys = ('mode', 'version', 'commit', 'source_dir', 'lib_dir', 'include_dir')
        args.output.write_text(''.join(f'{k}={evidence[k]}\n' for k in keys if k in evidence),
                               encoding='utf-8')
        print(f"Rust Core: {evidence['mode']} {evidence['version']} {evidence['commit']}",
              file=sys.stderr)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Core dependency verification failed: {error}\n')


if __name__ == '__main__':
    main()
