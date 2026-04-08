// This file shares some content with sample.rs for ambiguity tests.
// It also has unique content of its own.

fn voxcode_sample_fixture_alpha() -> u32 {
    let unique_identifier_9a3f2b = 42;
    unique_identifier_9a3f2b + 1
}

fn duplicate_only_function() {
    println!("this_content_only_in_duplicate_file");
}
