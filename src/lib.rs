use flate2::read::ZlibDecoder;
use std::io::Read;
use std::fmt;

type Fixed = fixed::FixedI32<fixed::frac::U2>;

const HEADER_SIZE: usize = 128;
const FRAME_HEADER_SIZE: usize = 16;

#[derive(Default, Debug)]
pub struct Header {
    file_size: u32,
    magic_number: u16,
    frames: u16,
    width: u16,
    height: u16,
    color_depth: ColorDepth,
    flags: u32,
    speed: u16,
    pallette_entry: u8,
    number_of_colors: u16,
    pixel_width: u8,
    pixel_height: u8,
}

#[derive(Debug)]
pub struct Frame {
    pub size: u32,
    pub magic_number: u16,
    pub old_chunks: u16,
    pub frame_duration: u16,
    pub new_chunks: u32,
    pub chunks: Vec<Chunk>,
    pub layers: Vec<Layer>,
}

impl Frame {
    pub fn new(header: &Header, raw: &[u8]) -> Frame {
        let mut frame = Frame{
            size: read_dword(&raw[0..]),
            magic_number: read_word(&raw[4..]),
            old_chunks: read_word(&raw[6..]),
            frame_duration: read_word(&raw[8..]),
            new_chunks: read_dword(&raw[12..]),
            chunks: Vec::new(),
            layers: Vec::new(),
        };

        let mut offset = FRAME_HEADER_SIZE;
        let chunk_count = if frame.new_chunks == 0 {
            frame.old_chunks as u32
        } else {
            frame.new_chunks
        };

        for _ in 0..chunk_count {
            let (chunk, size) = Chunk::new(header, &raw[offset..]);
            offset += size as usize;
            match chunk {
                Chunk::Layer(layer) => frame.layers.push(layer),
                Chunk::Cel(cel) => frame.layers[cel.layer_index() as usize].cels.push(cel),
                _ => frame.chunks.push(chunk),
            }
        }

        frame
    }
}

#[derive(Debug)]
pub struct Layer {
    flags: u16,
    layer_type: LayerType,
    child_level: u16,
    default_width: u16,
    default_height: u16,
    blend_mode: u16,
    opacity: u8,
    name: String,
    cels: Vec<Cel>,
}

#[derive(Debug)]
enum LayerType {
    Normal,
    Group,
}

impl From<u16> for LayerType {
    fn from(raw: u16) -> LayerType {
        match raw {
            0 => LayerType::Normal,
            1 => LayerType::Group,
            _ => panic!("Invalid layer type!"),
        }
    }
}

#[derive(Debug)]
pub enum Chunk {
    OldPallette,
    OtherOldPallette,
    Layer(Layer),
    Cel(Cel),
    CelExtra{
        flags: u32,
        x: Fixed,
        y: Fixed,
        width: Fixed,
        height: Fixed,
    },
    ColorProfile{
        profile_type: u16,
        flags: u16,
        gamma: Fixed,
        icc_size: u32,
        icc_data: Vec<u8>,
    },
    Mask{
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        mask_name: String,
        data: Vec<u8>,
    },
    FrameTags,
    Pallette{
        size: u32,
        first_color_index: u32,
        last_color_index: u32,
        entries: Vec<PalletteEntry>,
    },
    Slice{
        key_count: u32,
        flags: u32,
        name: String,
        keys: Vec<SliceKey>,
    },
    Path,
}

impl Chunk {
    fn new_layer(raw: &[u8]) -> Chunk {
        let (name, _) = read_string(&raw[16..]);
        let layer = Layer{
            flags: read_word(&raw[0..]),
            layer_type: LayerType::from(read_word(&raw[2..])),
            child_level: read_word(&raw[4..]),
            default_width: read_word(&raw[6..]),
            default_height: read_word(&raw[8..]),
            blend_mode: read_word(&raw[10..]),
            opacity: raw[12],
            // 3 unused bytes
            name,
            cels: Vec::new(),
        };

        Chunk::Layer(layer)
    }

    fn new_color_profile(raw: &[u8]) -> Chunk {
        Chunk::ColorProfile{
            profile_type: read_word(&raw[0..]),
            flags: read_word(&raw[2..]),
            gamma: read_fixed(&raw[4..]),
            // TODO (erik): Parse ICC data.
            icc_size: 0,
            icc_data: Vec::new(),
        }
    }

    fn new_mask(raw: &[u8]) -> Chunk {
        let width = read_word(&raw[4..]);
        let height = read_word(&raw[6..]);
        let (mask_name, offset) = read_string(&raw[8..]);
        let data_size = (height * ((width + 7)/8)) as usize;
        Chunk::Mask{
            x: read_short(&raw[0..]),
            y: read_short(&raw[2..]),
            width,
            height,
            mask_name,
            data: Vec::from(&raw[10 + offset..10 + offset + data_size]),
        }
    }

    fn new_cel(header: &Header, raw: &[u8]) -> Chunk {
        Chunk::Cel(Cel::new(header, &raw))
    }

    fn new_cel_extra(raw: &[u8]) -> Chunk {
        Chunk::CelExtra{
            flags: read_dword(&raw[0..]),
            x: read_fixed(&raw[4..]),
            y: read_fixed(&raw[8..]),
            width: read_fixed(&raw[12..]),
            height: read_fixed(&raw[16..])
        }
    }

    fn new_pallette(raw: &[u8]) -> Chunk {
        Chunk::Pallette{
            size: read_dword(&raw[0..]),
            first_color_index: read_dword(&raw[4..]),
            last_color_index: read_dword(&raw[8..]),
            // TODO (erik): Parse entries
            entries: Vec::new(),
        }
    }

    fn new_slice(raw: &[u8]) -> Chunk {
        let (name, _) = read_string(&raw[8..]);
        Chunk::Slice{
            key_count: read_dword(&raw[0..]),
            flags: read_dword(&raw[4..]),
            name: name,
            // TODO (erik): Parse keys.
            keys: Vec::new(),
        }
    }

    pub fn new(header: &Header, raw: &[u8]) -> (Chunk, u32) {
        let size = read_dword(&raw[0..]);
        let chunk_type = read_word(&raw[4..]);

        (match chunk_type {
            0x0004 => Chunk::OldPallette,
            0x0011 => Chunk::OtherOldPallette,
            0x2004 => Chunk::new_layer(&raw[6..]),
            0x2005 => Chunk::new_cel(header, &raw[6..size as usize]),
            0x2006 => Chunk::new_cel_extra(&raw[6..]),
            0x2007 => Chunk::new_color_profile(&raw[6..]),
            0x2016 => Chunk::new_mask(&raw[6..]),
            0x2017 => Chunk::Path,
            0x2018 => Chunk::FrameTags,
            0x2019 => Chunk::new_pallette(&raw[6..]),
            0x2022 => Chunk::new_slice(&raw[6..]),
            _ => panic!("Invalid chunk type!"),
        }, size)
    }
}

#[derive(Debug)]
pub struct CelBase {
    layer_index: u16,
    x: i16,
    y: i16,
    opacity: u8,
}

impl CelBase {
    fn new(raw: &[u8]) -> CelBase {
        CelBase{
            layer_index: read_word(&raw[0..]),
            x: read_short(&raw[2..]),
            y: read_short(&raw[4..]),
            opacity: raw[6],
        }
    }

    fn offset() -> usize {
        7
    }
}

#[derive(Debug)]
pub struct RawCel {
    base: CelBase,
    width: u16,
    height: u16,
    pixels: Vec<Pixel>,
}

impl RawCel {
    fn new(color_depth: &ColorDepth, raw: &[u8]) -> RawCel {
        let offset = CelBase::offset() + 9; // 7 for unused bytes, 2 for cel_type
        let width = read_word(&raw[offset..]) + 1;
        let height = read_word(&raw[offset+2..]) + 1;

        RawCel{
            base: CelBase::new(raw),
            width,
            height,
            pixels: Pixel::new_pixels(color_depth, width, height, &raw[offset+4..]),
        }
    }

    fn from_compressed(color_depth: &ColorDepth, raw: &[u8]) -> RawCel {
        let offset = CelBase::offset() + 9; // 7 for unused bytes, 2 for cel_type
        let width = read_word(&raw[offset..]);
        let height = read_word(&raw[offset+2..]);
        let mut data = Vec::with_capacity((width * height) as usize * color_depth.offset());
        ZlibDecoder::new(&raw[offset+4..]).read_to_end(&mut data).unwrap();

        RawCel{
            base: CelBase::new(raw),
            width,
            height,
            pixels: Pixel::new_pixels(color_depth, width, height, &data),
        }
    }

    // maps a pixel index to a target width, since RawCel pixel data is often smaller than the
    // canvas dimensions represented in the Header.
    fn map_pixel(&self, idx: usize, width: usize) -> usize {
        let row = idx / (self.width as usize);
        let col = idx % (self.width as usize);

        (row * width) + (col + self.base.x as usize)
    }
}

#[derive(Debug)]
pub struct LinkedCel {
    base: CelBase,
    frame_position: u16,
}

impl LinkedCel {
    fn new(raw: &[u8]) -> LinkedCel {
        LinkedCel{
            base: CelBase::new(raw),
            frame_position: read_word(&raw[CelBase::offset()..])
        }
    }
}

// unused
#[derive(Debug)]
pub struct CompressedCel {
    base: CelBase,
    width: u16,
    height: u16,
    data: Vec<u8>, // ZLIB compressed data
}

impl CompressedCel {
    fn new(raw: &[u8]) -> CompressedCel {
        let offset = CelBase::offset();
        let width = read_word(&raw[offset..]);
        let height = read_word(&raw[offset+2..]);

        CompressedCel{
            base: CelBase::new(raw),
            width,
            height,
            data: Vec::from(&raw[offset+4..]),
        }

    }
}

#[derive(Debug)]
pub enum Cel {
    Raw(RawCel),
    Linked(LinkedCel),
    Compressed(CompressedCel)
}

impl Cel {
    // fn new_raw(color_depth: &ColorDepth, raw: &[u8]) -> Cel {
    //     let width = read_word(&raw[0..]);
    //     let height = read_word(&raw[2..]);

    //     Cel::Raw{
    //         width,
    //         height,
    //         pixels: Pixel::new_pixels(&color_depth, width, height, raw),
    //     }
    // }

    // fn new_linked(raw: &[u8]) -> Cel {
    //     Cel::Linked{
    //         frame_position: read_word(&raw[0..]),
    //     }
    // }

    // fn new_compressed(color_depth: &ColorDepth, raw: &[u8]) -> Cel {
    //     let width =  read_word(&raw[0..]);
    //     let height = read_word(&raw[2..]);
    //     let mut data = Vec::with_capacity((width * height) as usize * color_depth.offset());
    //     ZlibDecoder::new(&raw[4..]).read_to_end(&mut data).unwrap();

    //     // TODO (erik): Is returning a decompressed, raw cel acceptable behaviour when we find it
    //     // compressed?
    //     Cel::Raw{
    //         width,
    //         height,
    //         pixels: Pixel::new_pixels(&color_depth, width, height, &data),
    //     }
    //     // Cel::Compressed{
    //     //     width,
    //     //     height,
    //     //     data,
    //     // }
    // }

    fn new(header: &Header, raw: &[u8]) -> Cel {
        // NOTE: Compressed Cels actually get returned as their decompressed Raw counterpart. If
        // actual compressed cels are needed, this match needs changed.
        let cel_type = read_word(&raw[7..]);
        match cel_type {
            0 => Cel::Raw(RawCel::new(&header.color_depth, raw)),
            1 => Cel::Linked(LinkedCel::new(raw)),
            2 => Cel::Raw(RawCel::from_compressed(&header.color_depth, raw)),
            _ => panic!("Invalid cel type!"),
        }
    }

    fn layer_index(&self) -> u16 {
        match self {
            Cel::Raw(c) => c.base.layer_index,
            Cel::Linked(c) => c.base.layer_index,
            Cel::Compressed(c) => c.base.layer_index,
        }
    }
}

#[derive(Debug)]
enum ColorDepth {
    RGBA,
    GrayScale,
    Indexed,
}

impl Default for ColorDepth {
    fn default() -> ColorDepth {
        ColorDepth::RGBA
    }
}

impl From<u16> for ColorDepth {
    fn from(word: u16) -> ColorDepth {
        match word {
            32 => ColorDepth::RGBA,
            16 => ColorDepth::GrayScale,
            8 => ColorDepth::Indexed,
            _ => panic!("Invalid color depth!"),
        }
    }
}

impl ColorDepth {
    fn offset(&self) -> usize {
        match self {
            ColorDepth::RGBA => 4,
            ColorDepth::GrayScale => 2,
            ColorDepth::Indexed => 1,
        }
    }
}

// #[derive(Debug)]
pub enum Pixel {
    RGBA{
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    },
    GrayScale{
        value: u8,
        alpha: u8,
    },
    Indexed{
        index: u8,
    }
}

impl fmt::Debug for Pixel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl Pixel {
    fn new_rgba(raw: &[u8]) -> Pixel {
        Pixel::RGBA{
            r: raw[0],
            g: raw[1],
            b: raw[2],
            a: raw[3],
        }
    }

    fn new_gray_scale(raw: &[u8]) -> Pixel {
        Pixel::GrayScale{
            value: raw[0],
            alpha: raw[1],
        }
    }

    fn new_indexed(raw: &[u8]) -> Pixel {
        Pixel::Indexed{
            index: raw[0],
        }
    }

    fn new(color_depth: &ColorDepth, raw: &[u8]) -> Pixel {
        match color_depth {
            ColorDepth::RGBA => Pixel::new_rgba(raw),
            ColorDepth::GrayScale => Pixel::new_gray_scale( raw),
            ColorDepth::Indexed => Pixel::new_indexed(raw),
        }
    }

    fn new_pixels(color_depth: &ColorDepth, width: u16, height: u16, raw: &[u8]) -> Vec<Pixel> {
        let pixel_fn = match color_depth {
            ColorDepth::RGBA => Pixel::new_rgba,
            ColorDepth::GrayScale => Pixel::new_gray_scale,
            ColorDepth::Indexed => Pixel::new_indexed,
        };

        let mut pixels = Vec::new();
        let mut offset = 0;
        for _ in 0..(width * height) {
            pixels.push(pixel_fn(&raw[offset..]));
            offset += color_depth.offset();
        }

        pixels
    }
}

#[derive(Debug)]
pub struct SliceKey {
    pub frame_number: u32,
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
    pub center_x: i64,
    pub center_y: i64,
    pub center_width: u32,
    pub center_height: u32,
    pub pivot_x: i64,
    pub pivot_y: i64,
}

#[derive(Debug, Default)]
pub struct PalletteEntry {
    pub flags: u16,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
    pub color_name: String,
}

#[derive(Default, Debug)]
pub struct Ase {
    pub header: Header,
    pub frames: Vec<Frame>,
}

impl Ase {
    pub fn new(raw: &[u8]) -> Ase {
        let header = Header::new(raw);
        let mut frames = Vec::new();
        let mut offset = HEADER_SIZE;
        for _ in 0..header.frames {
            let frame = Frame::new(&header, &raw[offset..]);
            offset += frame.size as usize;
            frames.push(frame);
        }

        Ase{
            header,
            frames,
        }

    }

    /// Renders the Ase structure into an array of pixel values. The final format of this data
    /// depends on the color depth defined in the Header.
    ///
    /// A render is generated by iterating over the cel chunks in each layer and applying the color
    /// data with the configured opacity. If there are multiple frames, this procedure is repeated
    /// for each frame generating a strip of frame data.
    pub fn render(&self) -> Vec<u8> {
        let width = self.header.width as usize;
        let height = self.header.height as usize;
        let color_depth = &self.header.color_depth;
        println!("Header width: {}, Header height: {}", width, height);
        let mut image_data: Vec<u8> = vec![0; width * height * color_depth.offset()];
        println!("Image data length: {}", image_data.len());

        for frame in &self.frames {
            for layer in &frame.layers {
                for cel in &layer.cels {
                    match cel {
                        Cel::Raw(c) => {
                            for (i, pixel) in c.pixels.iter().enumerate() {
                                match pixel {
                                    Pixel::RGBA{r, g, b, a} => {
                                        let idx = c.map_pixel(i, width) * color_depth.offset();
                                        image_data[idx] = *r;
                                        image_data[idx+1] = *g;
                                        image_data[idx+2] = *b;
                                        image_data[idx+3] = *a;
                                    },
                                    Pixel::GrayScale{value, alpha} => {
                                        image_data.push(*value);
                                        image_data.push(*alpha);
                                    },
                                    Pixel::Indexed{index} => image_data.push(*index),
                                }
                            }
                            println!("Found a raw cel!")
                        }
                        _ => println!("Non pixel cel"),
                    }
                }
            }
        }

        image_data
    }
}


impl Header {
    pub fn new(raw: &[u8]) -> Header {
        Header{
            file_size: read_dword(&raw[0..]),
            magic_number: read_word(&raw[4..]),
            frames: read_word(&raw[6..]),
            width: read_word(&raw[8..10]),
            height: read_word(&raw[10..12]),
            color_depth: ColorDepth::from(read_word(&raw[12..14])),
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

fn read_short(bytes: &[u8]) -> i16 {
    ((bytes[0] as i16) << 0) +
    ((bytes[1] as i16) << 8)
}

fn read_long(bytes: &[u8]) -> i32 {
    ((bytes[0] as i32) << 0) +
    ((bytes[1] as i32) << 8) +
    ((bytes[2] as i32) << 16) +
    ((bytes[3] as i32) << 24)
}

fn read_fixed(bytes: &[u8]) -> Fixed {
    Fixed::from_bits(read_long(bytes))
}

fn read_string(bytes: &[u8]) -> (String, usize) {
    let length = read_word(&bytes[0..]) as usize;
    // TODO (erik): Maybe make this safe? Not sure what to do with the error, though.
    let raw_str = unsafe { std::str::from_utf8_unchecked(&bytes[2..length + 2]) };
    (String::from(raw_str), length + 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::png::PNGEncoder;
    use std::fs;

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

    #[test]
    fn test_read_file() {
        let test_bytes = include_bytes!("../test.ase");
        let ase = Ase::new(test_bytes);
        println!("{:?}", ase);
    }

    #[test]
    fn test_render_image() {
        let test_bytes = include_bytes!("../test.ase");
        let ase = Ase::new(test_bytes);
        let image_data = ase.render();
        let file = fs::File::create("output.png").unwrap();
        let encoder = PNGEncoder::new(file);
        let width = ase.header.width as u32;
        let height = ase.header.height as u32;
        encoder.encode(&image_data, width, height, image::ColorType::RGBA(8)).unwrap();
    }
}
