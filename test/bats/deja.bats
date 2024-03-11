
setup() {
    load 'test_helper'
    deja_setup
}

@test "(no args)" {
  deja
  assert_handled_failure
}

@test "--help" {
  deja --help
  assert_success
}

@test "--version" {
  deja --version
  assert_success
}

@test "run" {
  deja run -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result"
}

@test "run --watch-path" {
  folder=$(folder_fixture folder)
  other_folder=$(folder_fixture other_folder)

  deja run --watch-path $folder -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run --watch-path $folder -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result"

  deja run --watch-path $other_folder -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result for different path"

  touch $folder/file
  deja run --watch-path $folder -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result when watched path changes"
}

@test "run --watch-scope" {
  deja run --watch-scope a -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run --watch-scope a -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result"

  deja run --watch-scope b -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result for different scope"

  deja run --watch-scope a --watch-scope b -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result for with extra scope"

  deja run --watch-scope a -- $test_command
  assert_success_with_uuid_matching $first_output "still returns result when called with original scope"
}

@test "run --watch-env" {
  ENV_A=1 deja run --watch-env ENV_A -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  ENV_A=1 deja run --watch-env ENV_A -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result"

  ENV_A=1 ENV_B=2 deja run --watch-env ENV_A -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result if other env set"

  ENV_A=2 deja run --watch-env ENV_A -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result with different env value"

  deja run --watch-env ENV_A -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result when env not set"
}

@test "run --look-back" {
  deja run -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  sleep 1

  deja run --look-back 5s -- $test_command
  assert_success_with_uuid_matching $first_output "returns previous result looking back 5s"

  deja run --look-back 1s -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result if cached result is too stale"

  fresh_output=$output

  deja run --look-back 1s -- $test_command
  assert_success_with_uuid_matching $fresh_output "new result is now cached for future calls"

  deja run --look-back 1s -- $test_command
  assert_success_with_uuid_matching $fresh_output "new result is also returned when no look back specified"
}

@test "run --cache-for" {
  deja run --cache-for 1s -- $test_command
  assert_success_with_uuid "runs command and returns result"

  first_output=$output

  deja run -- $test_command
  assert_success_with_uuid_matching $first_output "returns result when called within cache-for period"

  sleep 1

  deja run -- $test_command
  assert_success_with_uuid_not_matching $first_output "returns fresh result if cached result has expired"
}

@test "run --exclude-pwd" {
  folder=$(folder_fixture folder)

  deja run -- $test_command
  assert_success_with_uuid "runs command and returns result"

  output_without_flag=$output

  deja run --debug --exclude-pwd -- $test_command
  assert_success_with_uuid_not_matching $output_without_flag "generates different result when --exclude-pwd flag is set"

  output_with_flag=$output

  deja run --debug --exclude-pwd -- $test_command
  assert_success_with_uuid_matching $output_with_flag "returns previous result when --exclude-pwd flag is set"

  cd $folder

  deja run -- $test_command
  assert_success_with_uuid_not_matching $output_without_flag "returns different result when called without flag from different folder"
  assert_not_equal "$output" "$output_with_flag" "doesn't return output generated with flag when called without flag"


  deja run --debug --exclude-pwd -- $test_command
  assert_success_with_uuid_matching $output_with_flag "returns previous result from when called with flag from different folder"
}

@test "run (error: command not found)" {
  deja run -- unknown
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: command not found: unknown"
}

@test "run (error: permission denied to run command)" {
  deja run -- ./README.md
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: permission denied running command: ./README.md"
}

@test "run (error: unable to write to cache)" {
  deja run --cache /missing/folder -- $test_command
  assert_handled_failure "fails when unknown command"
  assert_equal "$stderr" "deja: unable to write to cache /missing/folder"
}

@test "run (error: unable to read from cache)" {
  deja run -- $test_command

  echo $DEJA_CACHE > .deja
  chmod -R 300 $DEJA_CACHE/*

  deja run -- $test_command

  assert_handled_failure "fails when unable to read cache entry"
  assert_regex "$stderr" "deja: unable to read cache entry $DEJA_CACHE/.*"
}

@test "run --look-back (error: invalid duration)" {
  deja run --look-back 1xyz -- $test_command
  assert_handled_failure "fails when duration can't be parsed"
  assert_equal "$stderr" "deja: invalid duration '1xyz', use values like 15s, 30m, 3h, 4d etc"
}

@test "run --cache-for (error: invalid duration)" {
  deja run --cache-for 1xyz -- $test_command
  assert_handled_failure "fails when duration can't be parsed"
  assert_equal "$stderr" "deja: invalid duration '1xyz', use values like 15s, 30m, 3h, 4d etc"
}

@test "run --cache-for (error: watch-path not found)" {
  deja run --watch-path missing -- $test_command
  assert_handled_failure "fails when --watch-path is missing"
  assert_equal "$stderr" "deja: watch path 'missing' not found"
}

@test "read" {
  deja read -- $test_command
  assert_handled_failure "fails when no result cached"

  deja run -- $test_command
  first_output=$output

  deja read -- $test_command
  assert_success_with_uuid_matching $first_output "returns cached result"
}

@test "read --cache-miss-exit-code" {
  deja read --cache-miss-exit-code 123 -- $test_command
  assert_handled_failure "fails when no result cached"
  assert_equal "$status" "123" "returns exit code specified when no result cached"
}

@test "force" {
  deja run -- $test_command

  first_output=$output

  deja force -- $test_command
  assert_success_with_uuid_not_matching $first_output "forces new result"

  forced_output=$output

  deja run -- $test_command
  assert_success_with_uuid_matching $forced_output "forced result now cached"
}

@test "remove" {
  deja run -- $test_command

  first_output=$output

  deja remove -- $test_command
  deja run -- $test_command

  assert_success_with_uuid_not_matching $first_output "removing result forces new result on next call"

  deja remove -- $test_command
  assert_success

  deja test -- $test_command
  assert_handled_failure "removing result removes it from cache"

  deja remove -- $test_command
  assert_handled_failure "removing result that doesn't exist fails"
}

@test "test" {
  deja test -- $test_command
  assert_handled_failure "fails when no result cached"

  deja run -- $test_command

  deja test -- $test_command
  assert_success "succeeds now result cached"

  deja remove -- $test_command

  deja test -- $test_command
  assert_handled_failure "fails when result removed"
}

@test "explain" {
  deja explain -- $test_command
  assert_success
}

@test "hash" {
  deja hash -- $test_command
  assert_success

  first_output=$output

  deja hash -- $test_command
  assert_equal $first_output $output "returns previous hash"

  deja hash --watch-path src -- $test_command
  assert_not_equal $first_output $output "returns different hash with different options"
}
