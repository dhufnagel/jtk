# Benchmark: jtk against palantir-java-format, Checkstyle and PMD

Measured on 17.11.2026 with jtk 0.2.1, `scripts/bench-tools.sh`.

## Setup

- **Machine:** AMD Ryzen 9 3900 (12 cores, 20 usable by the tools), Linux.
- **Tools:**
  - palantir-java-format 2.103.0, native image from Maven Central (`--palantir --dry-run`).
  - Checkstyle 14.4.0 (`checkstyle-all.jar`, OpenJDK 25).
  - PMD 7.29.0 (`pmd check`, OpenJDK 25).
- **Corpora:**
  - commons-lang, `src/main` and `src/test`, 646 files.
  - The JDK 25 sources from `lib/src.zip`, 15,227 files.
- **Same rules:** the linters run only the rules that jtk has an equivalent for, with the configurations of
  `scripts/compare-with-checkstyle.sh` (92 Checkstyle checks) and `scripts/compare-with-pmd.sh` (52 PMD rules). jtk
  runs the matching rule codes.
- **How the tools run:** every tool runs as its users run it: jtk, PMD and palantir-java-format in parallel on all
  cores, Checkstyle on one thread. Wall-clock time is what you wait for; CPU time (user + system) is the work done
  on all cores together.
- **Runs:** commons-lang is the best of 3 runs, the JDK one run.

## Formatting

| Corpus | palantir-java-format | jtk | Faster by |
|---|---|---|---|
| commons-lang (646 files) | 18.0 s (CPU 60 s) | 0.8 s (CPU 6.7 s) | 23× |
| JDK 25 (15,227 files) | 244 s (CPU 890 s) | 12.3 s (CPU 124 s) | 20× |

The output is the same byte for byte (see "Comparison with the original" in the README).

## Linting

| Corpus | Checkstyle (92 rules) | jtk, same rules | Faster by |
|---|---|---|---|
| commons-lang | 9.3 s (CPU 26 s) | 0.41 s (CPU 4.2 s) | 23× |
| JDK 25 | 163 s (CPU 303 s) | 9.4 s (CPU 101 s) | 17× |

| Corpus | PMD (52 rules) | jtk, same rules | Faster by |
|---|---|---|---|
| commons-lang | 5.1 s (CPU 58 s) | 0.33 s (CPU 3.9 s) | 16× |
| JDK 25 | 21.0 s (CPU 329 s) | 8.0 s (CPU 91 s) | 2.6× |

jtk with its own rule sets:

| Corpus | Default rules | All rules | `jtk check` (format and all rules) |
|---|---|---|---|
| commons-lang | 0.23 s (CPU 2.6 s) | 0.49 s (CPU 5.1 s) | 0.94 s (CPU 10 s) |
| JDK 25 | 4.9 s (CPU 62 s) | 10.4 s (CPU 121 s) | 20.0 s (CPU 218 s) |

`jtk check` takes less than formatting and linting separately, as both share the parse of each file. For
comparison, palantir-java-format, Checkstyle and PMD one after the other take 32 s on commons-lang and 428 s on the
JDK.

## Notes

- **Where the time goes:** a large part of the JVM tools' time on small projects is start-up and JIT warm-up. This
  explains why the lead on commons-lang is larger than on the JDK. PMD scales well over many files; Checkstyle uses
  one thread.
- **What the numbers do not include:** build-tool integrations (Gradle, Maven, Spotless) keep a JVM running between
  builds, which saves the start-up; they do not change how much work the formatting and the checks themselves take.
- **Results:** they are not compared here; `compare-with-checkstyle.sh` and `compare-with-pmd.sh` do that.

## Reproducing

```sh
scripts/bench-tools.sh <palantir-java-format-native-binary> <checkstyle-all.jar> <pmd-bin-dir> <source-dir> [runs]
```
