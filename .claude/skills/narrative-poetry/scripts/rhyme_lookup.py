#!/usr/bin/env python3
"""
Rhyme lookup tool using the CMU Pronouncing Dictionary.
Provides metrically-aware rhyme queries for poetry generation.

Usage:
    python rhyme_lookup.py <word> [--syllables N] [--stress-pattern PATTERN]

Examples:
    python rhyme_lookup.py night
    python rhyme_lookup.py night --syllables 2 --stress-pattern 10
    python rhyme_lookup.py desire --syllables 2 --stress-pattern 01
"""

import argparse
import sys

try:
    import pronouncing
except ImportError:
    print("ERROR: 'pronouncing' library not installed.", file=sys.stderr)
    print("Install with: pip install pronouncing", file=sys.stderr)
    sys.exit(1)


def get_stress_pattern(word: str) -> str | None:
    """Get the stress pattern for a word (0=unstressed, 1=primary, 2=secondary)."""
    phones = pronouncing.phones_for_word(word)
    if not phones:
        return None
    # Get stresses from first pronunciation
    return pronouncing.stresses(phones[0])


def get_syllable_count(word: str) -> int | None:
    """Get the syllable count for a word."""
    phones = pronouncing.phones_for_word(word)
    if not phones:
        return None
    return pronouncing.syllable_count(phones[0])


def matches_stress_pattern(word: str, pattern: str) -> bool:
    """Check if a word matches the given stress pattern."""
    word_pattern = get_stress_pattern(word)
    if word_pattern is None:
        return False
    # Normalize: treat 2 (secondary stress) as 0 for matching purposes
    # unless the pattern explicitly uses 2
    if '2' not in pattern:
        word_pattern = word_pattern.replace('2', '0')
    return word_pattern == pattern


def find_rhymes(word: str, syllables: int | None = None, stress_pattern: str | None = None) -> list[str]:
    """
    Find rhymes for a word with optional metrical constraints.

    Args:
        word: The word to find rhymes for
        syllables: If specified, only return rhymes with this many syllables
        stress_pattern: If specified, only return rhymes matching this stress pattern
                       (e.g., "01" for iambic, "10" for trochaic)

    Returns:
        List of rhyming words matching the constraints
    """
    rhymes = pronouncing.rhymes(word)

    if not rhymes:
        return []

    results = []
    for rhyme in rhymes:
        # Filter by syllable count
        if syllables is not None:
            count = get_syllable_count(rhyme)
            if count != syllables:
                continue

        # Filter by stress pattern
        if stress_pattern is not None:
            if not matches_stress_pattern(rhyme, stress_pattern):
                continue

        results.append(rhyme)

    return sorted(results)


def get_rhyme_info(word: str) -> dict:
    """Get pronunciation info for a word."""
    phones = pronouncing.phones_for_word(word)
    if not phones:
        return {"word": word, "found": False}

    return {
        "word": word,
        "found": True,
        "syllables": pronouncing.syllable_count(phones[0]),
        "stress_pattern": pronouncing.stresses(phones[0]),
        "phones": phones[0]
    }


def main():
    parser = argparse.ArgumentParser(
        description="Find rhymes with optional metrical constraints"
    )
    parser.add_argument("word", help="Word to find rhymes for")
    parser.add_argument(
        "--syllables", "-s",
        type=int,
        help="Filter rhymes to this syllable count"
    )
    parser.add_argument(
        "--stress-pattern", "-p",
        help="Filter rhymes to this stress pattern (e.g., '01' for iambic, '10' for trochaic)"
    )
    parser.add_argument(
        "--info", "-i",
        action="store_true",
        help="Show pronunciation info for the query word"
    )
    parser.add_argument(
        "--limit", "-l",
        type=int,
        default=50,
        help="Maximum number of rhymes to show (default: 50)"
    )

    args = parser.parse_args()
    word = args.word.lower()

    # Show word info if requested
    if args.info:
        info = get_rhyme_info(word)
        if info["found"]:
            print(f"Word: {info['word']}")
            print(f"Syllables: {info['syllables']}")
            print(f"Stress pattern: {info['stress_pattern']}")
            print(f"Phonemes: {info['phones']}")
            print()
        else:
            print(f"Word '{word}' not found in dictionary")
            print()

    # Find rhymes
    rhymes = find_rhymes(word, args.syllables, args.stress_pattern)

    if not rhymes:
        if args.syllables or args.stress_pattern:
            print(f"No rhymes found for '{word}' matching constraints:")
            if args.syllables:
                print(f"  - syllables: {args.syllables}")
            if args.stress_pattern:
                print(f"  - stress pattern: {args.stress_pattern}")
            print("\nTry relaxing constraints or querying without filters first.")
        else:
            print(f"No rhymes found for '{word}'")
            # Check if word is in dictionary at all
            if not pronouncing.phones_for_word(word):
                print(f"Note: '{word}' is not in the CMU dictionary")
        sys.exit(0)

    # Print results
    total = len(rhymes)
    displayed = rhymes[:args.limit]

    print(f"Rhymes for '{word}'", end="")
    if args.syllables:
        print(f" ({args.syllables} syllables)", end="")
    if args.stress_pattern:
        print(f" (stress: {args.stress_pattern})", end="")
    print(f": {total} found")
    print()

    # Group by syllable count for readability
    by_syllables: dict[int, list[str]] = {}
    for r in displayed:
        count = get_syllable_count(r) or 0
        by_syllables.setdefault(count, []).append(r)

    for syl_count in sorted(by_syllables.keys()):
        words = by_syllables[syl_count]
        print(f"{syl_count}-syllable: {', '.join(words)}")

    if total > args.limit:
        print(f"\n... and {total - args.limit} more (use --limit to see more)")


if __name__ == "__main__":
    main()
