use lightweight_pdf::*;

#[test]
fn measure_text_calculates_dimensions_and_line_count() {
    let style = TextStyle::new().size(12.0);
    let short = "Hello World";
    let m_short = measure_text_default(short, &style, 200.0).expect("measure should succeed");

    assert!(m_short.width > 0.0);
    assert!(m_short.height > 0.0);
    assert_eq!(m_short.lines, 1);

    // Wrapping: constrained width forces multiple lines
    let long = "This is a longer paragraph designed to wrap across several lines when given a narrow maximum width.";
    let m_wrap = measure_text_default(long, &style, 80.0).expect("measure should succeed");
    assert!(m_wrap.lines > 1);
    assert!(m_wrap.width <= 80.01);
    assert!(m_wrap.height > m_short.height);

    // Larger font size scales proportionally
    let style_large = TextStyle::new().size(24.0);
    let m_large = measure_text_default(short, &style_large, 200.0).expect("measure should succeed");
    assert!(m_large.width > m_short.width * 1.8);
    assert!(m_large.height > m_short.height * 1.8);
}

#[test]
fn measure_element_calculates_dimensions() {
    let text_elem = Element::from(Text::new("Sample text").size(14.0));
    let m_elem = measure_element_default(&text_elem, 300.0).expect("measure should succeed");
    assert!(m_elem.width > 0.0);
    assert!(m_elem.height > 0.0);

    // Table measurement: fills max_width by default
    let table = Table::new()
        .columns([TableColumn::fixed(50.0), TableColumn::fixed(50.0)])
        .rows(vec![
            vec![TableCell::new("Row 1 Col 1"), TableCell::new("Row 1 Col 2")],
            vec![TableCell::new("Row 2 Col 1"), TableCell::new("Row 2 Col 2")],
        ]);
    let table_elem = Element::from(table);
    let m_table = measure_element_default(&table_elem, 200.0).expect("table measure");
    assert_eq!(m_table.width, 200.0);
    assert!(m_table.height > 20.0);

    // Explicitly sized table
    let table_fixed = Table::new().width(100.0);
    let m_fixed = measure_element_default(&Element::from(table_fixed), 200.0).expect("table measure");
    assert_eq!(m_fixed.width, 100.0);
}
