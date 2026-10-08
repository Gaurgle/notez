//! Throwaway test for NZ-19: proves the CI gate turns red. Reverted in the
//! next commit on this branch.

#[test]
fn ci_red_check_fails_on_purpose() {
    panic!("deliberate red run for NZ-19");
}
