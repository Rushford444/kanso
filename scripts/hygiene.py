#!/usr/bin/env python3
"""Shared checks for public artifacts."""
import re

WORDS = [b"ar" + b"gos", b"jar" + b"vis", b"pr" + b"ime", b"lu" + b"is", b"miel" + b"anis"]
PATTERNS = [re.compile(rb"(?i)(?<![A-Za-z0-9])" + word + rb"(?![A-Za-z0-9])") for word in WORDS]
PATTERNS += [re.compile(rb"(?i)(?<![A-Za-z0-9])" + word + rb"s?(?![A-Za-z0-9])") for word in [b"la" + b"ne", b"wor" + b"ker"]]
PATTERNS += [re.compile(rb"(?<![A-Za-z0-9])RO" + rb"OT(?![A-Za-z0-9])")]
PATTERNS += [re.compile(rb"(?i)/home/" + b"node"), re.compile(rb"(?i)/Us" + b"ers/")]


def problems(name, content):
    return [f"{name}: forbidden pattern" for pattern in PATTERNS
            if pattern.search(name.encode("utf-8", "surrogateescape")) or pattern.search(content)]
