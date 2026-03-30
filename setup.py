"""Custom setup to produce platform-specific wheels (binary included)."""

import os
import sys
from setuptools import setup
from wheel.bdist_wheel import bdist_wheel


class PlatformWheel(bdist_wheel):
    """Force the wheel to be tagged as platform-specific (not pure Python)."""

    def finalize_options(self):
        super().finalize_options()
        self.root_is_pure = False

    def get_tag(self):
        python, abi, plat = super().get_tag()
        # We don't depend on a specific Python version or ABI
        return "py3", "none", plat


setup(cmdclass={"bdist_wheel": PlatformWheel})
