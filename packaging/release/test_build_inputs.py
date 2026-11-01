import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class BuildInputsTest(unittest.TestCase):
    def test_selected_commit_and_invalid_inputs(self):
        script = Path(__file__).with_name('build-inputs.py')
        sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'output'
            env = dict(os.environ, GITHUB_OUTPUT=str(output))
            subprocess.run(['python3', '-B', str(script), sha], env=env, check=True)
            self.assertIn('arch_image=archlinux@sha256:', output.read_text())
            before = output.read_text()
            for invalid in ('main', '0' * 40):
                result = subprocess.run(['python3', '-B', str(script), invalid], env=env,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(output.read_text(), before)
