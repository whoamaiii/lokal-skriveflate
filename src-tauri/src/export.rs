use anyhow::Result;

fn wrap_text(input: &str, max_width: usize) -> Vec<String> {
    let mut wrapped = Vec::new();

    for raw_line in input.lines() {
        let mut current = String::new();

        for word in raw_line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
                continue;
            }

            if current.len() + word.len() + 1 > max_width {
                wrapped.push(current.clone());
                current.clear();
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }

        if !current.is_empty() {
            wrapped.push(current);
        } else {
            wrapped.push(String::new());
        }
    }

    wrapped
}

fn escape_pdf_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

pub fn render_text_pdf(input: &str) -> Result<Vec<u8>> {
    let wrapped_lines = wrap_text(input, 88);
    let lines_per_page = 48;
    let pages = wrapped_lines
        .chunks(lines_per_page)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();

    let mut objects = Vec::new();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push(String::new());
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    let mut page_ids = Vec::new();

    for (index, page_lines) in pages.iter().enumerate() {
        let page_object_id = 4 + index * 2;
        let content_object_id = 5 + index * 2;
        page_ids.push(page_object_id);

        let mut stream = String::from("BT\n/F1 11 Tf\n14 TL\n48 760 Td\n");
        for line in page_lines {
            stream.push_str(&format!("({}) Tj\nT*\n", escape_pdf_text(line)));
        }
        stream.push_str("ET");

        let content = format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.as_bytes().len(),
            stream
        );

        let page = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            content_object_id
        );

        objects.push(page);
        objects.push(content);
    }

    objects[1] = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        page_ids
            .iter()
            .map(|id| format!("{} 0 R", id))
            .collect::<Vec<_>>()
            .join(" "),
        page_ids.len()
    );

    let mut output = Vec::new();
    output.extend_from_slice(b"%PDF-1.4\n%\xFF\xFF\xFF\xFF\n");

    let mut offsets = vec![0usize];

    for (index, object) in objects.iter().enumerate() {
        offsets.push(output.len());
        let object_string = format!("{} 0 obj\n{}\nendobj\n", index + 1, object);
        output.extend_from_slice(object_string.as_bytes());
    }

    let xref_start = output.len();
    output.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    output.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    output.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF",
            objects.len() + 1,
            xref_start
        )
        .as_bytes(),
    );

    Ok(output)
}
