use rusty_core::format_source;

#[test]
fn formats_construct_groups_inside_blocks() {
  let source = r"
fn main() {
  let foo = value();
  let bar = value();
  foo::bar();
  foo.bar();
  return 5;
}
";

  let expected = r"
fn main() {
  let foo = value();
  let bar = value();

  foo::bar();
  foo.bar();

  return 5;
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn formats_tail_expression_as_a_separate_group() {
  let source = r"
fn value() -> i32 {
  let foo = 5;
  foo
}
";

  let expected = r"
fn value() -> i32 {
  let foo = 5;

  foo
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn formats_nested_block_construct_groups() {
  let source = r"
fn main() {
  if enabled {
    let foo = 5;
    work(foo);
  }
}
";

  let expected = r"
fn main() {
  if enabled {
    let foo = 5;

    work(foo);
  }
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn leaves_comment_separated_constructs_unchanged() {
  let source = r"
fn main() {
  let foo = 5;
  // Keep this attached to the call.
  work(foo);
}
";

  assert_eq!(format_source(source), source);
}

#[test]
fn formats_conditionals_and_match_as_separate_groups() {
  let source = r"
fn main() {
  let value = read();
  call(value);
  if value > 5 {
    call(value);
  } else if value > 2 {
    other(value);
  } else {
    fallback();
  }
  match value {
    0 => zero(),
    _ => many(),
  }
  finish();
}
";

  let expected = r"
fn main() {
  let value = read();

  call(value);

  if value > 5 {
    call(value);
  } else if value > 2 {
    other(value);
  } else {
    fallback();
  }

  match value {
    0 => zero(),
    _ => many(),
  }

  finish();
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn keeps_macro_calls_with_action_group() {
  let source = r"
fn main() {
  let value = read();
  trace!(value);
  call(value);
}
";

  let expected = r"
fn main() {
  let value = read();

  trace!(value);
  call(value);
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn separates_multiline_entries_even_when_kind_matches() {
  let source = r"
fn main() {
  let foo = {
    value()
  };
  let bar = value();
  let baz = value();
  call(
    foo,
  );
  other(foo);
  let record = Config {
    foo,
    bar,
  };
  let final_value = value();
}
";

  let expected = r"
fn main() {
  let foo = {
    value()
  };

  let bar = value();
  let baz = value();

  call(
    foo,
  );

  other(foo);

  let record = Config {
    foo,
    bar,
  };

  let final_value = value();
}
";

  assert_eq!(format_source(source), expected);
}

#[test]
fn separates_adjacent_multiline_matches() {
  let source = r"
fn main() {
  match first {
    Some(value) => value,
    None => 0,
  }
  match second {
    Some(value) => value,
    None => 0,
  }
}
";

  let expected = r"
fn main() {
  match first {
    Some(value) => value,
    None => 0,
  }

  match second {
    Some(value) => value,
    None => 0,
  }
}
";

  assert_eq!(format_source(source), expected);
}
