#!/usr/bin/env python3
"""
Meter verification tool using the CMU Pronouncing Dictionary.
Checks scansion of poetic lines against expected metrical patterns.

Usage:
    python meter_check.py "<line>" [--expected-pattern PATTERN]

Examples:
    python meter_check.py "The curfew tolls the knell of parting day"
    python meter_check.py "The curfew tolls the knell of parting day" --expected-pattern 0101010101
"""

import argparse
import re
import sys

try:
    import pronouncing
except ImportError:
    print("ERROR: 'pronouncing' library not installed.", file=sys.stderr)
    print("Install with: pip install pronouncing", file=sys.stderr)
    sys.exit(1)


def clean_word(word: str) -> str:
    """Remove punctuation and lowercase a word."""
    return re.sub(r"[^a-zA-Z']", "", word).lower()


def get_word_stress(word: str) -> tuple[str | None, int | None]:
    """
    Get stress pattern and syllable count for a word.
    Returns (stress_pattern, syllable_count) or (None, None) if not found.
    """
    phones = pronouncing.phones_for_word(word)
    if not phones:
        return None, None
    return pronouncing.stresses(phones[0]), pronouncing.syllable_count(phones[0])


def analyze_line(line: str) -> dict:
    """
    Analyze the meter of a line of poetry.

    Returns a dict with:
        - words: list of (word, stress_pattern, syllable_count)
        - full_pattern: concatenated stress pattern for the line
        - syllable_count: total syllables
        - unknown_words: words not found in dictionary
    """
    words = line.split()
    results = []
    full_pattern = ""
    total_syllables = 0
    unknown_words = []

    for word in words:
        clean = clean_word(word)
        if not clean:
            continue

        stress, count = get_word_stress(clean)
        if stress is None:
            unknown_words.append(clean)
            # Make a guess based on word length
            # This is imperfect but better than nothing
            if len(clean) <= 3:
                guessed_stress = "1"  # Assume monosyllabic
                guessed_count = 1
            else:
                # Very rough heuristic
                guessed_count = max(1, len(clean) // 3)
                guessed_stress = "01" * (guessed_count // 2)
                if guessed_count % 2:
                    guessed_stress += "1"
            results.append((clean, f"?{guessed_stress}", guessed_count))
            full_pattern += guessed_stress
            total_syllables += guessed_count
        else:
            results.append((clean, stress, count))
            full_pattern += stress
            total_syllables += count

    return {
        "words": results,
        "full_pattern": full_pattern,
        "syllable_count": total_syllables,
        "unknown_words": unknown_words
    }


def pattern_matches(actual: str, expected: str, strict: bool = False) -> tuple[bool, str]:
    """
    Check if actual stress pattern matches expected.

    In non-strict mode (default):
    - 2 (secondary stress) can match 0 or 1
    - Allows common metrical substitutions

    Returns (matches, explanation)
    """
    if len(actual) != len(expected):
        return False, f"Length mismatch: got {len(actual)} syllables, expected {len(expected)}"

    if strict:
        if actual == expected:
            return True, "Exact match"
        else:
            mismatches = []
            for i, (a, e) in enumerate(zip(actual, expected)):
                if a != e:
                    mismatches.append(f"position {i+1}: got {a}, expected {e}")
            return False, f"Mismatches: {', '.join(mismatches)}"

    # Non-strict matching
    mismatches = []
    for i, (a, e) in enumerate(zip(actual, expected)):
        if a == e:
            continue
        # Secondary stress (2) can often work as either stressed or unstressed
        if a == '2' and e in '01':
            continue
        # In practice, unstressed syllables sometimes take stress
        # and vice versa - this is metrical variation, not error
        # But we still flag it
        mismatches.append(f"position {i+1}: got {a}, expected {e}")

    if not mismatches:
        return True, "Match (with secondary stress flexibility)"
    elif len(mismatches) <= 2:
        return True, f"Acceptable variation: {', '.join(mismatches)}"
    else:
        return False, f"Too many mismatches: {', '.join(mismatches)}"


def identify_meter(pattern: str) -> str:
    """Attempt to identify the meter from a stress pattern."""
    length = len(pattern)

    # Common meters
    meters = {
        # Iambic patterns (01)
        "0101010101": "iambic pentameter",
        "01010101": "iambic tetrameter",
        "010101": "iambic trimeter",
        "01010101010101": "iambic heptameter (fourteener)",

        # Trochaic patterns (10)
        "1010101010": "trochaic pentameter",
        "10101010": "trochaic tetrameter",
        "101010": "trochaic trimeter",

        # Dactylic patterns (100)
        "100100100100": "dactylic tetrameter",
        "100100100": "dactylic trimeter",

        # Anapestic patterns (001)
        "001001001001": "anapestic tetrameter",
        "001001001": "anapestic trimeter",
    }

    if pattern in meters:
        return meters[pattern]

    # Check for approximate matches
    # Iambic?
    if length == 10:
        iambic_diff = sum(1 for i, c in enumerate(pattern) if c != "01"[i % 2])
        if iambic_diff <= 2:
            return f"roughly iambic pentameter ({iambic_diff} variations)"

    if length == 8:
        iambic_diff = sum(1 for i, c in enumerate(pattern) if c != "01"[i % 2])
        if iambic_diff <= 2:
            return f"roughly iambic tetrameter ({iambic_diff} variations)"

    # Ballad meter check (8686 or 8888)
    if length in [6, 8]:
        return f"{length}-syllable line (possible ballad meter)"

    return f"unidentified ({length} syllables)"


def main():
    parser = argparse.ArgumentParser(
        description="Check meter/scansion of a line of poetry"
    )
    parser.add_argument("line", help="Line of poetry to analyze")
    parser.add_argument(
        "--expected-pattern", "-e",
        help="Expected stress pattern (e.g., '0101010101' for iambic pentameter)"
    )
    parser.add_argument(
        "--strict", "-s",
        action="store_true",
        help="Require exact pattern match (no substitutions allowed)"
    )

    args = parser.parse_args()

    # Analyze the line
    analysis = analyze_line(args.line)

    print(f"Line: {args.line}")
    print(f"Syllables: {analysis['syllable_count']}")
    print(f"Stress pattern: {analysis['full_pattern']}")
    print()

    # Show word-by-word breakdown
    print("Word breakdown:")
    for word, stress, count in analysis["words"]:
        marker = " (?)" if stress.startswith("?") else ""
        print(f"  {word}: {stress.lstrip('?')} ({count} syl){marker}")
    print()

    # Warn about unknown words
    if analysis["unknown_words"]:
        print(f"WARNING: Words not in dictionary (stress guessed): {', '.join(analysis['unknown_words'])}")
        print()

    # Identify meter
    meter = identify_meter(analysis["full_pattern"])
    print(f"Identified as: {meter}")

    # Check against expected pattern if provided
    if args.expected_pattern:
        print()
        matches, explanation = pattern_matches(
            analysis["full_pattern"],
            args.expected_pattern,
            strict=args.strict
        )
        if matches:
            print(f"PASS: {explanation}")
        else:
            print(f"FAIL: {explanation}")
            sys.exit(1)


if __name__ == "__main__":
    main()
