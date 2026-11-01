# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel
"""Verify a published crate pair on crates.io itself, as a new user sees it.

Holds no credential and no GitHub token. A published version cannot change, so a
failure here means releasing a new version.
"""

import argparse
import json
import os
import time

from ci import REGISTRY_CRATES, REGISTRY_SHARED, Cargo, install
from ci_contract import CRATES, REPOSITORY, lock, matrix
from ci_support import ROOT, require, write
from crates_io import CRATES_API, OWNER, fetch, owners, wait_published
from registry_consumer import published_consumers

DOCS = 'https://docs.rs/crate'
DOCS_WAIT_SECONDS = 1800
DOCS_POLL_SECONDS = 30
WORK = ROOT / 'build/p'
OUTPUT = ROOT / 'build/published-consumers.json'
REGISTRY_OUTPUT = ROOT / 'build/published-registry.json'


def metadata(crate, version, expected):
    """Compare what crates.io shows with the identity that the `package` check requires."""
    listed = json.loads(fetch(f'{CRATES_API}/{crate}'))['crate']
    served = json.loads(fetch(f'{CRATES_API}/{crate}/{version}'))['version']
    identity = {**REGISTRY_SHARED, **REGISTRY_CRATES[crate]}
    actual = {'checksum': served['checksum'], 'yanked': served['yanked'], 'license': served['license'],
              'repository': served['repository'], 'description': served['description'],
              'edition': served['edition'], 'rust-version': served['rust_version'],
              'links': served['lib_links'], 'features': served['features'],
              'keywords': sorted(listed['keywords']), 'categories': sorted(listed['categories'])}
    wanted = {'checksum': expected, 'yanked': False, 'license': 'MIT',
              'repository': f'https://github.com/{REPOSITORY}',
              'keywords': sorted(identity['keywords']), 'categories': sorted(identity['categories']),
              **{key: identity[key] for key in ('description', 'edition', 'rust-version', 'links', 'features')}}
    differing = sorted(key for key, value in wanted.items() if actual[key] != value)
    require(not differing, f'crates.io shows other {crate} metadata: {differing}')
    logins = owners(crate) or set()
    require(OWNER in logins, f'{crate} on crates.io is not owned by {OWNER}')
    return {**actual, 'owners': sorted(logins),
            'url': f'https://crates.io/crates/{crate}/{version}'}


def documented(crate, version):
    """Whether docs.rs has built this version; it builds from a queue, so this can take a while."""
    data = fetch(f'{DOCS}/{crate}/{version}/status.json')
    return data is not None and json.loads(data).get('doc_status') is True


def wait_documented(crate, version, deadline=None):
    if deadline is None:
        deadline = time.monotonic() + DOCS_WAIT_SECONDS
    while not documented(crate, version):
        require(time.monotonic() < deadline, f'docs.rs has no documentation of {crate} {version}')
        time.sleep(DOCS_POLL_SECONDS)


def registry(version, packages):
    REGISTRY_OUTPUT.unlink(missing_ok=True)
    results = {}
    for crate in CRATES:
        wait_published(crate, version, packages[crate])
        results[crate] = metadata(crate, version, packages[crate])
    deadline = time.monotonic() + DOCS_WAIT_SECONDS
    for crate in CRATES:
        wait_documented(crate, version, deadline)
        results[crate]['documentation'] = f'https://docs.rs/{crate}/{version}'
    write(REGISTRY_OUTPUT, {'version': version, 'status': 'success', 'crates': results})


def consumers(version, packages, configuration_id):
    OUTPUT.unlink(missing_ok=True)
    configuration = next(item for item in matrix('full') if item['id'] == configuration_id)
    install(configuration)
    environment = Cargo(configuration, WORK, False).env
    results = published_consumers(version, packages, lock(True), WORK, environment)
    write(OUTPUT, {'version': version, 'configuration': configuration_id, 'consumers': results})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('registry', 'consumers'))
    args = parser.parse_args()
    published = json.loads(os.environ['PUBLISHED'])
    if args.command == 'registry':
        registry(published['version'], published['packages'])
    else:
        consumers(published['version'], published['packages'], os.environ['CONFIGURATION'])


if __name__ == '__main__':
    main()
