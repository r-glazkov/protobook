use std::fs::File;
use std::io::BufReader;
use uuid::Uuid;
use protobook::{Book, Date};

#[test]
fn deserialize_fb2() {
    let file =
        File::open("examples/books/Макаренко Антон — Педагогическая поэма. Полная версия.fb2")
            .unwrap();
    let reader = BufReader::new(file);
    let book: fb2::FictionBook = quick_xml::de::from_reader(reader).unwrap();
    let book_id = Uuid::new_v4();
    let binary_ids = book
        .binaries
        .iter()
        .map(|binary| (binary.id.clone(), Uuid::new_v4()))
        .collect();
    let book = Book::from_fb2(book, book_id, &binary_ids);

    assert_eq!(book.id, book_id.to_string());
    assert_eq!(book.short_title, "Педагогическая поэма. Полная версия");
    assert_eq!(book.date, Some(Date { iso_date: "1936-01-01".to_string(), display_date: "1936".to_string() }));
}
