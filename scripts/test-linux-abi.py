#!/usr/bin/env python3
import pathlib
import runpy
import unittest

abi = runpy.run_path(str(pathlib.Path(__file__).with_name('check-linux-abi.py')))


class GlibcRequirementsTest(unittest.TestCase):
    def test_optional_new_symbols_do_not_raise_minimum(self):
        required, weak = abi['glibc_versions']('''Version needs section '.gnu.version_r' contains 1 entry:
  0x0010: Name: GLIBC_2.35  Flags: none  Version: 2
  0x0020: Name: GLIBC_2.39  Flags: WEAK  Version: 3
''')
        self.assertEqual(required, {'2.35'})
        self.assertEqual(weak, {'2.39'})

    def test_definitions_and_symbol_index_are_not_requirements(self):
        required, weak = abi['glibc_versions']('''Version symbols section '.gnu.version' contains 1 entry:
  000: 2 (GLIBC_2.42)
Version definition section '.gnu.version_d' contains 1 entry:
  0x0010: Name: GLIBC_2.42  Flags: none  Version: 2
Version needs section '.gnu.version_r' contains 1 entry:
  0x0010: Name: GLIBC_2.34  Flags: none  Version: 2
''')
        self.assertEqual(required, {'2.34'})
        self.assertFalse(weak)

    def test_strong_requirement_wins_even_if_also_optional_elsewhere(self):
        required, weak = abi['glibc_versions']('''Version needs section '.gnu.version_r' contains 2 entries:
  0x0010: Name: GLIBC_2.38  Flags: WEAK  Version: 2
  0x0020: Name: GLIBC_2.38  Flags: none  Version: 3
''')
        self.assertIn('2.38', required)
        self.assertIn('2.38', weak)

    def test_versions_use_numeric_comparison(self):
        self.assertEqual(max(['2.9', '2.35', '2.2.5'], key=abi['version_key']), '2.35')

    def test_private_glibc_dependency_is_rejected(self):
        with self.assertRaises(ValueError):
            abi['glibc_versions']("Version needs section '.gnu.version_r' contains 1 entry:\n"
                                  '  Name: GLIBC_PRIVATE  Flags: none  Version: 2')

    def test_non_numeric_abi_requirement_is_not_silently_ignored(self):
        with self.assertRaises(ValueError):
            abi['glibc_versions']("Version needs section '.gnu.version_r' contains 1 entry:\n"
                                  '  Name: GLIBC_ABI_DT_RELR  Flags: none  Version: 2')


if __name__ == '__main__':
    unittest.main()
