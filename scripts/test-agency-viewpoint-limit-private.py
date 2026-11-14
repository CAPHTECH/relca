#!/usr/bin/env python3
"""Execute only the new private acceptance tests, rejecting an empty selection."""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
NAMES = {
    "cli_all_budget_families_scale_only_work_and_cells",
    "d_each_viewpoint_limit_is_recorded_warned_and_block_local",
    "d_legacy_gate_twelve_boundary_has_two_work_records_and_warnings",
    "g8_invalid_ir_is_not_a_budget_exhaustion",
    "g3_viewpoint_domain_is_not_the_main_possible_results_domain",
    "g6_scale_never_multiplies_contexts_paths_or_depth",
    "s1_s2_generated_matrix_matches_frozen_enumerator_bytes_without_regression",
    "s1_s2_all_fixture_invocations_match_reference_without_regression",
    "s2_gate_twenty_matches_sufficient_budget_reference_bytes",
    "g7_responsibility_and_party_rederive_with_the_ir_scale",
}


def main():
    assert int(subprocess.check_output(["df", "--output=avail", "-B1", "."], cwd=ROOT).splitlines()[-1]) >= 15 * 1024**3
    env = dict(os.environ, CARGO_NET_OFFLINE="true", TMPDIR=".tmp", CARGO_PROFILE_DEV_DEBUG="0", CARGO_INCREMENTAL="0")
    command = ["cargo", "test", "-p", "relca-core", "--lib", "agency_viewpoint_limit_acceptance::"]
    listed = subprocess.run([*command, "--", "--list", "--format", "terse"], cwd=ROOT, env=env, capture_output=True, text=True, check=False)
    sys.stdout.write(listed.stderr)
    if listed.returncode:
        sys.stdout.write(listed.stdout)
        return listed.returncode
    found = {line.removesuffix(": test").split("::")[-1] for line in listed.stdout.splitlines() if line.endswith(": test")}
    if found != NAMES:
        print(f"RED private adapter census: missing={sorted(NAMES-found)} extra={sorted(found-NAMES)}")
        return 1
    result = subprocess.run([*command, "--", "--nocapture", "--test-threads=1"], cwd=ROOT, env=env, check=False)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
