use super::*;
use std::io::Cursor;

struct Chunks<R> {
    inner: R,
    size: usize,
}
impl<R: Read> Read for Chunks<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let length = bytes.len().min(self.size);
        self.inner.read(&mut bytes[..length])
    }
}

fn reference(bytes: &[u8], pattern: &ContentPattern, accents: bool) -> bool {
    BufReader::new(Cursor::new(bytes))
        .lines()
        .map_while(Result::ok)
        .any(|line| {
            pattern.is_match(&if accents {
                crate::utils::remove_accents(&line)
            } else {
                line
            })
        })
}

#[test]
fn streaming_agrees_with_line_regex_for_unicode_assertions_flags_and_chunks() {
    let patterns = [
        "",
        ".",
        ".*",
        "^$",
        "^a+$",
        "a.*z",
        "^a.*z$",
        r"\Aa.*z\z",
        r"a\nz",
        r"(?s)a.*z",
        r"(?m)^a.*$",
        r"(?Rm)^a$",
        r"\r$",
        r"\b\w+\b",
        r"\B",
        r"(?-u:\B)",
        r"\b{start}\w+\b{end}",
        r"\b{start-half}",
        r"\b{end-half}",
        r"(?-u:\b)é",
        "ÉTÉ",
        r"(?:a|ab)*?z",
        r"(a+)+z",
        r"(a?)*z",
        r"[^x]*z",
        r"\p{Greek}+",
        r"a{2,7}z",
        r"(?i)résumé",
        "resume",
        "e\u{05b0}\u{05b1}",
    ];
    let texts = [
        "",
        "\n",
        "\r\n",
        "\n\n",
        "\r",
        "a\r",
        "a\r\n",
        "a\rz\n",
        "a\nz",
        "aaaaz",
        "zaaaa",
        "xaaaazx",
        "aaa\nzzz\n",
        "é",
        "été",
        "ÉTÉ",
        " Résumé ",
        "re\u{0301}sume\u{0301}",
        "αβγ",
        "a🦀z",
        "e\u{05b1}\u{05b0}",
        "e\u{05b1}\u{034f}\u{05b0}",
        "각",
    ];
    for case_sensitive in [false, true] {
        for expression in patterns {
            let pattern = ContentPattern::new(expression, case_sensitive).unwrap();
            for accents in [false, true] {
                for text in texts {
                    let expected = reference(text.as_bytes(), &pattern, accents);
                    for limit in [0, 3, 64] {
                        for size in [1, 2, 7] {
                            let actual = contains(
                                Chunks {
                                    inner: Cursor::new(text.as_bytes()),
                                    size,
                                },
                                &pattern,
                                accents,
                                &AtomicBool::new(false),
                                limit,
                            )
                            .unwrap();
                            assert_eq!(actual, expected, "pattern={expression:?}, text={text:?}, case={case_sensitive}, accents={accents}, limit={limit}, chunk={size}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_utf8_and_read_errors_do_not_publish_partial_line_matches() {
    let pattern = ContentPattern::new("needle|^$", true).unwrap();
    for text in [
        b"needle\xff".as_slice(),
        b"needle\xf0\x9f",
        b"\xff\nneedle\n",
        b"needle\xe2\x28\xa1\n",
    ] {
        for size in [1, 3, 1024] {
            assert!(contains(
                Chunks {
                    inner: Cursor::new(text),
                    size
                },
                &pattern,
                false,
                &AtomicBool::new(false),
                0
            )
            .is_err());
        }
    }
    for text in [b"needle\n\xff".as_slice(), b"\n\xff", b"needle\r\n\xff"] {
        assert!(contains(
            Cursor::new(text),
            &pattern,
            false,
            &AtomicBool::new(false),
            0
        )
        .unwrap());
    }
    struct FailAfter {
        cursor: Cursor<&'static [u8]>,
    }
    impl Read for FailAfter {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let read = self.cursor.read(bytes)?;
            if read == 0 {
                Err(io::Error::other("injected read failure"))
            } else {
                Ok(read)
            }
        }
    }
    assert!(contains(
        FailAfter {
            cursor: Cursor::new(b"needle")
        },
        &pattern,
        false,
        &AtomicBool::new(false),
        0
    )
    .is_err());
}

#[test]
fn unbounded_regex_keeps_state_across_a_long_line_and_matches_its_end() {
    let text = format!("a{}z", "é".repeat(1024 * 1024));
    for (expression, expected) in [
        (r"\Aa.*z\z", true),
        (r"\Aa.*q\z", false),
        (r"éz\z", true),
        (r"ez\z", false),
    ] {
        let pattern = ContentPattern::new(expression, true).unwrap();
        assert_eq!(
            text_reader_contains_content(
                Cursor::new(text.as_bytes()),
                &pattern,
                false,
                &AtomicBool::new(false)
            ),
            expected
        );
    }
    let pattern = ContentPattern::new(r"\Aae+z\z", true).unwrap();
    assert!(text_reader_contains_content(
        Cursor::new(text.as_bytes()),
        &pattern,
        true,
        &AtomicBool::new(false)
    ));
}

#[test]
fn long_combining_sequences_keep_stable_nfd_order_even_across_removed_starters() {
    assert!(text_reader_contains_content(
        Cursor::new("\u{0301}".repeat(40000)),
        &ContentPattern::new("^$", true).unwrap(),
        true,
        &AtomicBool::new(false)
    ));
    let text = format!(
        "e{}\u{034f}{}z",
        "\u{05b1}\u{05b0}".repeat(6000),
        "\u{05b2}\u{05b0}".repeat(5000)
    );
    // Hebrew marks are intentionally retained by the existing accent filter.
    for expression in [
        r"\Ae\u{05b0}+\u{05b1}+\u{05b0}+\u{05b2}+z\z",
        "absent",
        r"\u{05b1}\u{05b0}",
    ] {
        let pattern = ContentPattern::new(expression, true).unwrap();
        let expected = reference(text.as_bytes(), &pattern, true);
        assert_eq!(
            contains(
                Chunks {
                    inner: Cursor::new(text.as_bytes()),
                    size: 127
                },
                &pattern,
                true,
                &AtomicBool::new(false),
                0
            )
            .unwrap(),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn cancellation_after_a_streaming_match_and_inside_normalization_discards_the_result() {
    use std::cell::Cell;
    struct Cancel<'a> {
        reads: Cell<usize>,
        cancel: &'a AtomicBool,
        normalize: bool,
    }
    impl Read for Cancel<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let reads = self.reads.get() + 1;
            self.reads.set(reads);
            if reads == 40 {
                self.cancel.store(true, Ordering::Relaxed);
            }
            if self.normalize {
                let chunk = "\u{05b1}\u{05b0}".as_bytes();
                let length = bytes.len() / chunk.len() * chunk.len();
                for item in bytes[..length].chunks_exact_mut(chunk.len()) {
                    item.copy_from_slice(chunk);
                }
                Ok(length)
            } else {
                bytes.fill(b'a');
                Ok(bytes.len())
            }
        }
    }
    for normalize in [false, true] {
        let cancel = AtomicBool::new(false);
        let reader = Cancel {
            reads: Cell::new(0),
            cancel: &cancel,
            normalize,
        };
        assert!(!text_reader_contains_content(
            reader,
            &ContentPattern::new("a|absent", true).unwrap(),
            normalize,
            &cancel
        ));
        assert!(cancel.load(Ordering::Relaxed));
    }
}

#[test]
fn interrupted_reads_are_retried_as_with_bufread_lines() {
    struct Interrupted {
        reads: usize,
        inner: Cursor<&'static [u8]>,
    }
    impl Read for Interrupted {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.reads += 1;
            if self.reads.is_multiple_of(2) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let count = bytes.len().min(2);
            self.inner.read(&mut bytes[..count])
        }
    }
    let pattern = ContentPattern::new(r"\Aneedle\z", true).unwrap();
    for limit in [0, 64] {
        assert!(contains(
            Interrupted {
                reads: 0,
                inner: Cursor::new(b"needle\r\n")
            },
            &pattern,
            false,
            &AtomicBool::new(false),
            limit
        )
        .unwrap());
    }
}

#[test]
fn generated_unicode_inputs_keep_boolean_equivalence_with_the_existing_regex() {
    let alphabet = [
        'a', 'z', ' ', '\r', '\n', 'É', '\u{0301}', '\u{05b0}', '\u{05b1}', '\u{034f}', 'أ', '각',
        '🦀', 'α', '_', '1',
    ];
    let mut seed = 0x1234_5678_u64;
    let mut texts = Vec::new();
    for index in 0..300 {
        let mut text = String::new();
        for _ in 0..index % 96 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(alphabet[((seed >> 32) as usize) % alphabet.len()]);
        }
        texts.push(text);
    }
    for expression in [
        r"^a.*z$",
        r"\bα\b",
        r"\B",
        r"(?-u:\B)",
        r"\b{start-half}a",
        r"\b{end-half}",
        r"(?Rm)^.*z$",
        r"[\w\s]*É",
        r"(?s)a.*z",
        r"(?:a|a?z)*$",
        r"[\u{05b0}-\u{05b2}]+",
        r"\A.*\z",
    ] {
        let pattern = ContentPattern::new(expression, false).unwrap();
        for accents in [false, true] {
            for (index, text) in texts.iter().enumerate() {
                let expected = reference(text.as_bytes(), &pattern, accents);
                let actual = contains(
                    Chunks {
                        inner: Cursor::new(text.as_bytes()),
                        size: index % 7 + 1,
                    },
                    &pattern,
                    accents,
                    &AtomicBool::new(false),
                    0,
                )
                .unwrap();
                assert_eq!(
                    actual, expected,
                    "{expression:?} {text:?} accents={accents}"
                );
            }
        }
    }
}
