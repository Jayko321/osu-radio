"""Read .SRCINFO versions without sourcing package-controlled shell code."""
import re
import subprocess


def package_version(text):
    values = {}
    for line in text.splitlines():
        match = re.fullmatch(r'\s*(epoch|pkgver|pkgrel) = (\S+)\s*', line)
        if match:
            key, value = match.groups()
            if key in values and values[key] != value:
                raise ValueError('ambiguous package version')
            values[key] = value
    if not {'pkgver', 'pkgrel'} <= values.keys():
        raise ValueError('missing package version')
    epoch = values.get('epoch', '0')
    if not epoch.isdecimal() or not re.fullmatch(r'[0-9]+(?:\.[0-9]+)?', values['pkgrel']):
        raise ValueError('invalid epoch or pkgrel')
    return f"{epoch}:{values['pkgver']}-{values['pkgrel']}"


def newer(new, old):
    comparison = subprocess.check_output(['vercmp', package_version(new), package_version(old)], text=True).strip()
    return int(comparison) > 0


if __name__ == '__main__':
    import sys
    from pathlib import Path
    if not newer(Path(sys.argv[1]).read_text(), Path(sys.argv[2]).read_text()):
        print('Already published an equal or newer package; skipping.')
        sys.exit(3)
