//! Linux x86-64 unoptimized frame budget from the pre-Q5 executable, not a
//! larger RUST_MIN_STACK. Requires the development toolchain's nm/objdump.
#![cfg(all(target_os = "linux", target_arch = "x86_64", debug_assertions))]
use relca_core::{lower, reference, syntax};
use std::process::Command;

#[test]
fn m2_reference_recursive_frame_does_not_exceed_pre_priority_20072_bytes() {
    let src = r#"model Frame version "0.2" { type R = enum { ok }
        actor A { capacity 1; capability c() -> R { duration fixed 1min; result nondet { ok } } }
        process P { r = A.c(); end done }
        bounds { cases 2; live_tasks 2; clock 5min } arrivals fixed [0min, 0min]
        verify safety no_resource_overcommit }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    assert_eq!(reference::explore(&m).stop, reference::StopReason::Complete);
    let exe = std::env::current_exe().unwrap();
    let nm = Command::new("nm")
        .args(["-C", "--defined-only"])
        .arg(&exe)
        .output()
        .unwrap();
    assert!(nm.status.success());
    let symbols = String::from_utf8(nm.stdout).unwrap();
    let matches: Vec<_> = symbols
        .lines()
        .filter(|l| {
            let name = l.splitn(3, ' ').last().unwrap_or("");
            name.contains("relca_core::reference::arrivals::Tree") && name.ends_with("::walk_inner")
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "must measure the linked interpreter, not an absent symbol"
    );
    let at = u64::from_str_radix(matches[0].split_whitespace().next().unwrap(), 16).unwrap();
    let asm = Command::new("objdump")
        .args([
            "-d",
            &format!("--start-address={at}"),
            &format!("--stop-address={}", at + 256),
        ])
        .arg(&exe)
        .output()
        .unwrap();
    assert!(asm.status.success());
    let assembly = String::from_utf8(asm.stdout).unwrap();
    let mut bytes = 0;
    for line in assembly.lines() {
        let Some(instruction) = line.split('\t').nth(2) else {
            continue;
        };
        let words: Vec<_> = instruction.split_whitespace().collect();
        if words.first() == Some(&"mov") && words.get(1).is_some_and(|v| v.starts_with('%')) {
            break;
        }
        if words.first() == Some(&"sub")
            && let Some(amount) = words
                .get(1)
                .and_then(|v| v.strip_prefix("$0x"))
                .and_then(|v| v.strip_suffix(",%rsp"))
        {
            bytes += u64::from_str_radix(amount, 16).unwrap();
        }
    }
    eprintln!("walk_inner rsp reservation: {bytes} bytes; maximum 20072");
    assert!(bytes > 0, "unrecognized prologue: {assembly}");
    assert!(
        bytes <= 20072,
        "recursive frame {bytes} exceeds pre-Q5 limit 20072"
    );
}
