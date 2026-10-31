import os
from pathlib import Path
import subprocess
import unittest

HELPER = Path(__file__).with_name('graphics-compat.sh')

class GraphicsCompatibility(unittest.TestCase):
    def env_result(self, wayland, vendors, existing=None):
        env = os.environ.copy()
        env.pop('WEBKIT_DISABLE_DMABUF_RENDERER', None)
        if existing is not None:
            env['WEBKIT_DISABLE_DMABUF_RENDERER'] = existing
        return subprocess.check_output(['sh', '-c', '. "$1"; oma_graphics_compat "$2" "$3"; printf "%s" "${WEBKIT_DISABLE_DMABUF_RENDERER-unset}"', 'test', str(HELPER), wayland, vendors], env=env, text=True)

    def test_nvidia_wayland_uses_compatibility(self):
        self.assertEqual(self.env_result('yes', '0x8086\n0x10de'), '1')

    def test_other_gpus_and_x11_keep_defaults(self):
        self.assertEqual(self.env_result('yes', '0x8086'), 'unset')
        self.assertEqual(self.env_result('no', '0x10de'), 'unset')

    def test_explicit_values_including_empty_are_preserved(self):
        for value in ('0', '1', ''):
            self.assertEqual(self.env_result('yes', '0x10de', value), value)

if __name__ == '__main__':
    unittest.main()
