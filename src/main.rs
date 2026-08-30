use tri::{BasicFilter, GenericFilter};

fn main() {
    let base = "/home/fabrice/Sources/exercism/rust";
    let extension = "rs";

    let filter = BasicFilter::new(base, 3, extension);

    let groups = filter.get_files();
    for (index, group) in groups.enumerate() {
        println!("===== GROUP {index}");
        println!();
        for entry in group {
            println!("{}", entry.path().display());
        }
    }
}
