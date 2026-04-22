#!/usr/bin/env python3

import sys

partial = sys.argv[2]

options = ["pull", "push", "run", "panic"]

for option in options:
    if option.startswith(partial):
        print(option)
