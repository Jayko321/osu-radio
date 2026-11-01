"""Prepare a package recipe; never execute a recipe while SSH credentials are present."""
import hashlib
import re
from pathlib import Path


def prepare(recipe, variant, revision, version=None, archive=None):
    if not re.fullmatch(r'[0-9]+', revision):
        raise ValueError('invalid publication revision')
    if variant == 'git':
        recipe = re.sub(r'pkgver\(\) \{.*?\n\}', '', recipe, flags=re.S)
        recipe = recipe.replace('#branch=main', '#branch=dev')
        pkgver = '0.r' + revision
        recipe = 'epoch=1\n' + recipe
    else:
        identifier = r'(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)'
        match = re.fullmatch(
            r'((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))'
            rf'(?:-({identifier}(?:\.{identifier})*))?'
            r'(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?', version or '')
        if not match:
            raise ValueError('invalid release version')
        # Arch forbids hyphens; build metadata does not affect SemVer precedence.
        pkgver = match[1] + ('pre' + match[2].replace('-', '_') if match[2] else '')
        recipe = re.sub(r'^_version=.*$', '_version=' + version, recipe, flags=re.M)
        recipe = re.sub(r"^sha256sums=.*$", "sha256sums=('" + hashlib.sha256(Path(archive).read_bytes()).hexdigest() + "')", recipe, flags=re.M)
    recipe = re.sub(r'^pkgver=.*$', 'pkgver=' + pkgver, recipe, flags=re.M)
    return re.sub(r'^pkgrel=.*$', 'pkgrel=1', recipe, flags=re.M)


if __name__ == '__main__':
    import sys
    variant, revision, output, *extra = sys.argv[1:]
    import os
    source = Path(os.environ.get('AUR_RECIPE_TEMPLATE', 'packaging/arch/osu-radio-' + variant + '/PKGBUILD'))
    Path(output).write_text(prepare(source.read_text(), variant, revision, *extra))
