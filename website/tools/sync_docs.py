"""Bundle repository documentation without changing the source game files."""
from pathlib import Path
import re
import shutil

SITE = Path(__file__).resolve().parents[1]
ROOT = SITE.parent
TARGET = SITE / 'content' / 'en'
TARGET.mkdir(parents=True, exist_ok=True)

sources = [ROOT / 'README.md', ROOT / 'ARCHITECTURE.md', *sorted((ROOT / 'docs').glob('*.md')),
           *sorted((ROOT / 'assets' / 'equipment').glob('*.md')),
           *sorted((ROOT / 'assets' / 'cables').glob('*.md'))]
for source in sources:
    text = source.read_text()
    if source.name == 'README.md':
        text = re.sub(r'Standalone Linux and Windows client downloads.*?See \[game-ssh instructions\]',
                      'Standalone Linux and Windows client release downloads are available in the\n[download section](/#downloads).\nSee [game-ssh instructions]', text, flags=re.S)
        text = re.sub(r'## Download a build\n.*?The Linux build includes',
                      '## Download a release\n\nChoose Windows or Linux in the [download section](/#downloads), then download\n'
                      'the game ZIP. The standalone game-ssh client and matching SHA-256 checksums\n'
                      'are listed there when published. If a package is unavailable, no release\n'
                      'file has been published for that platform yet.\n\n'
                      'Extract the ZIP and keep the executable and `assets` folder together.\n'
                      'On Linux, run `chmod +x cloud-provider-sim` if needed, then launch\n'
                      '`./cloud-provider-sim`. On Windows, launch `cloud-provider-sim.exe`.\n'
                      'The game saves `cloud-provider-save.db` in its current working directory.\n\n'
                      'The Linux build includes', text, flags=re.S)
        text = re.sub(r' GitHub sign-in with\nrepository read access.*?can be shared directly\.', '', text, flags=re.S)
    if source.name == 'GAME_SSH.md':
        text = re.sub(r'Download a ZIP.*?Linux packages are built',
                      'Download the standalone client ZIP and its matching `.zip.sha256` file\n'
                      'from the [download section](/#downloads). Select Windows or Linux.\n'
                      'Only published release files are offered; unavailable packages are marked\n'
                      'clearly. The full game ZIP already includes game-ssh.\n\nLinux packages are built', text, flags=re.S)
    (TARGET / source.name).write_text(text)
for directory in ('images', 'screenshots'):
    shutil.copytree(ROOT / 'docs' / directory, SITE / 'public' / 'docs-assets' / directory, dirs_exist_ok=True)
print(f'Bundled {len(sources)} English documents and their images. Russian translations are preserved.')
