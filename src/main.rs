use std::env;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    let out_fd_str = args.get(2).cloned().unwrap_or_default();
    let mut zipfile = args.get(3).cloned().unwrap_or_default();

    if zipfile.is_empty() {
        if let Some(fallback) = get_zipfile_from_env() {
            zipfile = fallback;
        }
    }

    // export LD_LIBRARY_PATH=/system/lib64:/system/lib:/vendor/lib64:/vendor/lib:$LD_LIBRARY_PATH
    let current_ld = env::var("LD_LIBRARY_PATH").unwrap_or_default();
    let new_ld = if current_ld.is_empty() {
        "/system/lib64:/system/lib:/vendor/lib64:/vendor/lib".to_string()
    } else {
        format!(
            "/system/lib64:/system/lib:/vendor/lib64:/vendor/lib:{}",
            current_ld
        )
    };
    env::set_var("LD_LIBRARY_PATH", &new_ld);

    // Obfuscate the branding headers using obfstr crate
    ui_print(&out_fd_str, obfstr::obfstr!("Target: Xiaomi/mivendor_sm6375_global/mivendor:11/RKQ1.230210.001/rsyd12240458:user/release-keys"));
    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));
    ui_print(&out_fd_str, obfstr::obfstr!("   MIUI V14.0.9.0.UMBEUXM | Android 14    "));
    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));

    let bin_tmp = "/tmp/bin";
    if let Err(_) = fs::create_dir_all(bin_tmp) {
        abort(&out_fd_str, obfstr::obfstr!("Failed to create /tmp/bin"));
    }

    // Extract bin/* using system unzip (which is always present in TWRP recovery).
    // This is safe because the bin/ folder is tiny and avoids the 32-bit ZIP limit.
    let _ = Command::new("unzip")
        .args(&["-o", &zipfile, "bin/*", "-d", "/tmp/"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // chmod +x $BIN_TMP/lptools $BIN_TMP/bootctl $BIN_TMP/unzip_64
    let _ = Command::new("chmod")
        .args(&[
            "+x",
            &format!("{}/lptools", bin_tmp),
            &format!("{}/bootctl", bin_tmp),
            &format!("{}/unzip_64", bin_tmp),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Determine the available 64-bit extraction tool (unzip_64)
    let path_unzip64 = find_unzip64_path(bin_tmp);

    let lptool = format!("{}/lptools", bin_tmp);
    let bctl = format!("{}/bootctl", bin_tmp);
    let rprop = "/system/bin/resetprop";

    ui_print(&out_fd_str, obfstr::obfstr!("Check for status mount vendor..."));

    // umount /vendor > /dev/null 2>&1
    let umount_status = Command::new("umount")
        .arg("/vendor")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if umount_status {
        ui_print(&out_fd_str, obfstr::obfstr!("Aosp rom, no need to remount it."));
    } else {
        ui_print(&out_fd_str, obfstr::obfstr!("detected as vendor meme"));
        ui_print(&out_fd_str, obfstr::obfstr!("Force unmount it..."));

        // umount -l /vendor > /dev/null 2>&1
        let _ = Command::new("umount")
            .args(&["-l", "/vendor"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // umount -f /vendor > /dev/null 2>&1
        let _ = Command::new("umount")
            .args(&["-f", "/vendor"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // if grep -qs "/vendor " /proc/mounts; then
        let is_vendor_mounted = fs::read_to_string("/proc/mounts")
            .map(|m| m.contains("/vendor "))
            .unwrap_or(false);

        if is_vendor_mounted {
            ui_print(&out_fd_str, obfstr::obfstr!("!! Force Unmount it."));
            let _ = Command::new("umount")
                .args(&["-l", "/vendor"])
                .status();
        } else {
            ui_print(&out_fd_str, obfstr::obfstr!("unmount vendor are success."));
        }
    }

    // CUR_SLOT_NUM=$($BCTL get-current-slot)
    let cur_slot_output = Command::new(&bctl)
        .arg("get-current-slot")
        .output();

    let cur_slot_num = match cur_slot_output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => {
            abort(&out_fd_str, obfstr::obfstr!("Failed to get current slot from bootctl"));
        }
    };

    let (l_cur, l_suffix, u_suffix, target_idx) = if cur_slot_num == "0" {
        ("a", "b", "B", 1)
    } else {
        ("b", "a", "A", 0)
    };
    let target_group = format!("qti_dynamic_partitions_{}", l_suffix);

    ui_print(&out_fd_str, &format!("{} {} ({})", obfstr::obfstr!("Currently on slot"), l_cur, cur_slot_num));
    ui_print(&out_fd_str, &format!("{} {} ({}). {}", obfstr::obfstr!("Target flash on slot"), u_suffix, target_idx, obfstr::obfstr!("OK.")));

    // $RPROP ro.boot.slot_suffix "_$L_SUFFIX"
    let _ = Command::new(rprop)
        .args(&["ro.boot.slot_suffix", &format!("_{}", l_suffix)])
        .status();

    ui_print(&out_fd_str, &format!("{} {} ...", obfstr::obfstr!("Force delete partition on"), l_cur));
    let logical_parts = ["mi_ext", "odm", "product", "system", "system_ext", "vendor"];

    for p in &logical_parts {
        let t_part = format!("{}_{}", p, l_suffix);
        let c_part = format!("{}_{}", p, l_cur);

        for slot in &["0", "1"] {
            let _ = Command::new(&lptool)
                .args(&["--slot", slot, "--unmap", &t_part])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = Command::new(&lptool)
                .args(&["--slot", slot, "--unmap", &c_part])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }

        for slot in &["0", "1"] {
            let _ = Command::new(&lptool)
                .args(&["--slot", slot, "--remove", &t_part])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = Command::new(&lptool)
                .args(&["--slot", slot, "--remove", &c_part])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    // $LPTOOL --slot 0 --clear-cow > /dev/null 2>&1
    // $LPTOOL --slot 1 --clear-cow > /dev/null 2>&1
    for slot in &["0", "1"] {
        let _ = Command::new(&lptool)
            .args(&["--slot", slot, "--clear-cow"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    for part in &logical_parts {
        let img_path = format!("images/{}.img", part);
        let target_part = format!("{}_{}", part, l_suffix);

        // 7z l "$ZIPFILE" "$IMG_PATH"
        let unzip_l_status = Command::new(&path_unzip64)
            .args(&["l", &zipfile, &img_path])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if unzip_l_status {
            // Get exact size of the image file
            let size_output = Command::new(&path_unzip64)
                .args(&["l", &zipfile, &img_path])
                .output();

            let mut size_bytes: u64 = 0;
            if let Ok(out) = size_output {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(parsed_size) = parse_unzip64_size(&text, &img_path) {
                    size_bytes = parsed_size;
                }
            }

            if size_bytes == 0 {
                abort(&out_fd_str, &format!("{} {}", obfstr::obfstr!("Could not determine size of"), img_path));
            }

            let size_mb = size_bytes / 1048576;
            ui_print(&out_fd_str, &format!("{} {} [{} MB]...", obfstr::obfstr!("Flashing"), target_part, size_mb));

            // $LPTOOL --slot $TARGET_IDX --suffix "_$L_SUFFIX" --group "$TARGET_GROUP" --create "$TARGET_PART" "$SIZE"
            let create_status = Command::new(&lptool)
                .args(&[
                    "--slot", &target_idx.to_string(),
                    "--suffix", &format!("_{}", l_suffix),
                    "--group", &target_group,
                    "--create", &target_part,
                    &size_bytes.to_string()
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if !create_status {
                // $LPTOOL --slot $TARGET_IDX --suffix "_$L_SUFFIX" --group "$TARGET_GROUP" --resize "$TARGET_PART" "$SIZE"
                let resize_status = Command::new(&lptool)
                    .args(&[
                        "--slot", &target_idx.to_string(),
                        "--suffix", &format!("_{}", l_suffix),
                        "--group", &target_group,
                        "--resize", &target_part,
                        &size_bytes.to_string()
                    ])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

                if !resize_status {
                    abort(&out_fd_str, &format!("{} {}, {}!", obfstr::obfstr!("Can't Allocate"), target_part, obfstr::obfstr!("It may not be suitable for you")));
                }
            }

            // $LPTOOL --slot $TARGET_IDX --suffix "_$L_SUFFIX" --group "$TARGET_GROUP" --map "$TARGET_PART"
            let _ = Command::new(&lptool)
                .args(&[
                    "--slot", &target_idx.to_string(),
                    "--suffix", &format!("_{}", l_suffix),
                    "--group", &target_group,
                    "--map", &target_part
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            // sleep 1
            thread::sleep(Duration::from_secs(1));

            // if [ ! -b "/dev/block/mapper/$TARGET_PART" ]; then abort...
            let mapper_path = format!("/dev/block/mapper/{}", target_part);
            let is_block_device = fs::metadata(&mapper_path)
                .map(|meta| meta.file_type().is_block_device())
                .unwrap_or(false);

            if !is_block_device {
                abort(&out_fd_str, &format!("{} {}, {}", obfstr::obfstr!("missing"), target_part, obfstr::obfstr!("file are corrupted")));
            }

            // unzip_64 x -so "$ZIPFILE" "$IMG_PATH" | direct Rust write using 8MB buffer
            let mut unzip_child = Command::new(&path_unzip64)
                .args(&["x", "-so", &zipfile, &img_path])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap_or_else(|_| abort(&out_fd_str, obfstr::obfstr!("Failed to spawn unzip_64")));

            let mut unzip_stdout = unzip_child.stdout.take().unwrap();

            let mut dest_file = fs::OpenOptions::new()
                .write(true)
                .open(&mapper_path)
                .unwrap_or_else(|_| abort(&out_fd_str, &format!("{} {}", obfstr::obfstr!("Failed to open mapper partition:"), mapper_path)));

            // High performance direct buffer copy (8MB sequential writes)
            let copy_result = copy_with_large_buffer(&mut unzip_stdout, &mut dest_file);
            if let Err(e) = copy_result {
                abort(&out_fd_str, &format!("{} {}: {}", obfstr::obfstr!("Write error on"), part, e));
            }

            // Single sync to disk at the end of the write operation to optimize NAND write speed
            if let Err(_) = dest_file.sync_all() {
                abort(&out_fd_str, &format!("{} {}", obfstr::obfstr!("Fsync failed on"), part));
            }

            let unzip_status = unzip_child.wait().map(|s| s.success()).unwrap_or(false);
            if !unzip_status {
                abort(
                    &out_fd_str,
                    &format!(
                        "{} {}, {}",
                        obfstr::obfstr!("Fail on"),
                        part,
                        obfstr::obfstr!("you're need to reboot recovery or reflash while in slot B")
                    ),
                );
            }

            // $LPTOOL --unmap "$TARGET_PART"
            let _ = Command::new(&lptool)
                .args(&["--unmap", &target_part])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));
    ui_print(&out_fd_str, obfstr::obfstr!("   Flashing boot, vendor_boot, and dtbo       "));
    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));

    let static_parts = ["boot", "vendor_boot", "dtbo"];
    for spart in &static_parts {
        let simg_path = format!("images/{}.img", spart);
        let target_spart = format!("{}_{}", spart, l_suffix);

        // 7z l "$ZIPFILE" "$SIMG_PATH"
        let unzip_l_status = Command::new(&path_unzip64)
            .args(&["l", &zipfile, &simg_path])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if unzip_l_status {
            // unzip_64 x -so "$ZIPFILE" "$SIMG_PATH" | direct Rust write using 8MB buffer
            let mut unzip_child = Command::new(&path_unzip64)
                .args(&["x", "-so", &zipfile, &simg_path])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap_or_else(|_| abort(&out_fd_str, obfstr::obfstr!("Failed to spawn unzip_64")));

            let mut unzip_stdout = unzip_child.stdout.take().unwrap();
            let dest_path = format!("/dev/block/by-name/{}", target_spart);

            if let Ok(mut dest_file) = fs::OpenOptions::new().write(true).open(&dest_path) {
                let _ = copy_with_large_buffer(&mut unzip_stdout, &mut dest_file);
                let _ = dest_file.sync_all();
            }

            let _ = unzip_child.wait();
        }
    }

    ui_print(&out_fd_str, &format!("{} {}...", obfstr::obfstr!("Switch to slot"), u_suffix));

    // $BCTL set-active-boot-slot $TARGET_IDX
    let _ = Command::new(&bctl)
        .args(&["set-active-boot-slot", &target_idx.to_string()])
        .status();

    // $RPROP ro.boot.slot_suffix "_$L_CUR"
    let _ = Command::new(rprop)
        .args(&["ro.boot.slot_suffix", &format!("_{}", l_cur)])
        .status();

    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));
    ui_print(&out_fd_str, obfstr::obfstr!("   Done And Don't Complain about Bug   "));
    ui_print(&out_fd_str, obfstr::obfstr!("--------------------------------------------"));
    std::process::exit(0);
}

fn ui_print(out_fd: &str, message: &str) {
    if out_fd.is_empty() {
        println!("{}", message);
        return;
    }
    let path = format!("/proc/self/fd/{}", out_fd);
    if let Ok(mut file) = fs::OpenOptions::new().write(true).open(&path) {
        let _ = writeln!(file, "ui_print {}", message);
        let _ = writeln!(file, "ui_print ");
        let _ = file.flush();
    } else {
        println!("{}", message);
    }
}

fn abort(out_fd: &str, message: &str) -> ! {
    ui_print(out_fd, obfstr::obfstr!("--------------------------------------------"));
    ui_print(out_fd, &format!("{}: {}", obfstr::obfstr!("!! Error"), message));
    ui_print(out_fd, obfstr::obfstr!("--------------------------------------------"));
    std::process::exit(1);
}

fn get_zipfile_from_env() -> Option<String> {
    for (key, value) in env::vars() {
        if key.to_lowercase().contains("zip") || value.to_lowercase().contains("zip") {
            return Some(value);
        }
    }
    None
}

fn find_unzip64_path(bin_tmp: &str) -> String {
    let path = format!("{}/unzip_64", bin_tmp);
    if std::path::Path::new(&path).exists() {
        path
    } else {
        // Fallback to system unzip if unzip_64 is not found
        "unzip".to_string()
    }
}

fn parse_unzip64_size(stdout: &str, img_path: &str) -> Option<u64> {
    for line in stdout.lines() {
        if line.contains(img_path) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                if let Ok(size) = parts[3].parse::<u64>() {
                    return Some(size);
                }
            }
        }
    }
    None
}

// Highly optimized copy with a large 8MB heap buffer. Disables repeated fsync overhead
// and eliminates sub-process dd context switching for maximum UFS/eMMC write speed.
fn copy_with_large_buffer<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> std::io::Result<u64> {
    let mut buffer = vec![0u8; 8 * 1024 * 1024]; // 8MB sequential write buffer
    let mut total_bytes = 0u64;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => {
                writer.write_all(&buffer[..n])?;
                total_bytes += n as u64;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    writer.flush()?;
    Ok(total_bytes)
}
