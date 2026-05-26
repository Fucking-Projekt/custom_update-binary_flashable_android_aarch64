use std::env;
use std::fs::File;
use std::io::{self, Write, BufReader, BufWriter};
use zip::ZipArchive;
use sevenz_rust::Password;

// Constants for Buffering
const BUFFER_SIZE: usize = 2 * 1024 * 1024; // 2MB Buffer for faster flashing

fn set_high_priority() {
    unsafe {
        // Set priority to -20 (Highest priority in Linux)
        // This ensures the process gets maximum attention from Big Cores
        // without completely locking out Little Cores.
        libc::setpriority(libc::PRIO_PROCESS, 0, -20);
        eprintln!("[INFO] Process priority set to High (-20)");
    }
}

fn main() -> io::Result<()> {
    // Boost priority instead of hard-locking cores
    set_high_priority();

    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        eprintln!("Usage: unzip_64 [l|x] [-slt|-so] <archive> <file>");
        std::process::exit(1);
    }

    let cmd = &args[1];
    let opt = &args[2];
    let archive_path = &args[3];
    let target_file = &args[4];

    if cmd == "l" {
        if archive_path.to_lowercase().ends_with(".7z") {
            let file = File::open(archive_path)?;
            let len = file.metadata()?.len();
            let mut sz = sevenz_rust::SevenZReader::new(file, len, Password::empty())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            sz.for_each_entries(|entry, _| {
                if entry.name() == target_file {
                    println!("Size = {}", entry.size());
                }
                Ok(true)
            }).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        } else {
            let file = File::open(archive_path)?;
            let mut zip = ZipArchive::new(file)?;
            let size = match zip.by_name(target_file) {
                Ok(f) => f.size(),
                Err(_) => 0,
            };
            if size > 0 {
                println!("Size = {}", size);
            }
        }
    } else if cmd == "x" && opt == "-so" {
        let mut stdout = BufWriter::with_capacity(BUFFER_SIZE, io::stdout());
        
        if archive_path.to_lowercase().ends_with(".7z") {
             sevenz_rust::decompress_file_with_extract_fn(archive_path, "", |entry, reader, _| {
                if entry.name() == target_file {
                    let mut buf_reader = BufReader::with_capacity(BUFFER_SIZE, reader);
                    io::copy(&mut buf_reader, &mut stdout).expect("Failed to copy data");
                    stdout.flush().expect("Failed to flush stdout");
                    Ok(true)
                } else {
                    Ok(false)
                }
            }).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        } else {
            let file = File::open(archive_path)?;
            let mut zip = ZipArchive::new(file)?;
            let mut z_file = zip.by_name(target_file)?;
            let mut buf_reader = BufReader::with_capacity(BUFFER_SIZE, &mut z_file);
            io::copy(&mut buf_reader, &mut stdout)?;
            stdout.flush()?;
        }
    }

    Ok(())
}
