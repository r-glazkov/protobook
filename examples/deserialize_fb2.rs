use std::fs::File;
use std::io::BufReader;
use uuid::Uuid;
use protobook::Book;

fn main() {
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

    println!("{}", book.id);
    println!("{}", book.short_title);
    println!("{}", book.date.as_ref().unwrap().iso_date);
    println!("{}", book.date.as_ref().unwrap().display_date);
}
