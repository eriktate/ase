#[derive(Default, Debug)]
struct Header {
    file_size: u32,
    magic_number: u16,
    frames: u16,
    width: u16,
    height: u16,
    color_depth: u16,
    flags: u32,
    speed: u16,
    pallette_entry: u8,
    number_of_colors: u16,
    pixel_width: u8,
    pixel_height: u8,
}

struct Frame {
    size: u32,
    magic_number: u16,
    old_chunks: u16,
    frame_duration: u16,
    new_chunks: u32,
}

struct Chunk {
    size: u32,
    chunk_type: u16,
    // chunk: variable based on chunk type
}

struct Ase {
    header: Header,
    Frames: Vec<Frame>,
}

impl Header {
    fn new(raw: &[u8]) -> Header {
        Header{
            file_size: read_dword(&raw[0..]),
            magic_number: read_word(&raw[4..]),
            frames: read_word(&raw[6..]),
            width: read_word(&raw[8..10]),
            height: read_word(&raw[10..12]),
            color_depth: read_word(&raw[12..14]),
            flags: read_dword(&raw[14..18]),
            speed: read_word(&raw[18..20]),
            pallette_entry: raw[24],
            number_of_colors: read_word(&raw[28..30]),
            pixel_width: raw[30],
            pixel_height: raw[31],
        }
    }
}

fn read_dword(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 0) +
    ((bytes[1] as u32) << 8) +
    ((bytes[2] as u32) << 16) +
    ((bytes[3] as u32) << 24)
}

fn read_word(bytes: &[u8]) -> u16 {
    ((bytes[0] as u16) << 0) +
    ((bytes[1] as u16) << 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_read_dword() {
        let test_bytes: Vec<u8> = vec![1, 0, 0, 0];
        let dword = read_dword(&test_bytes[0..4]);
        assert_eq!(dword, 1);
    }

    #[test]
    fn test_read_word() {
        let one: Vec<u8> = vec![1, 0];
        let one_word = read_word(&one[0..2]);
        let two_fifty_six: Vec<u8> = vec![0, 1];
        let two_fifty_six_word = read_word(&two_fifty_six[0..2]);
        assert_eq!(one_word, 1);
        assert_eq!(two_fifty_six_word, 256);
    }

    #[test]
    fn test_new_header() {
        let test_bytes = include_bytes!("../test.ase");
        let header = Header::new(test_bytes);
        println!("{:?}", header);
    }
}
