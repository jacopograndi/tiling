/// Decompress a zlib compressed slice
/// https://datatracker.ietf.org/doc/html/rfc1951
pub fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, DecompressResult> {
    let _cmf = data[0];
    let flg = data[1];
    let fdict = flg >> 5 & 1;
    let skip_dict = (2 + (fdict * 4)) as usize;
    decompress(&data[skip_dict..(data.len() - 4)])
}

/// Decompress a zlib inflate slice
/// https://datatracker.ietf.org/doc/html/rfc1951
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, DecompressResult> {
    let mut decoder = Decoder::from_bytes(data);
    let mut output: Vec<u8> = vec![];
    for _ in 0..LARGE_ITERATION_COUNT {
        let bfinal = decoder.next_bits(1);
        let btype = decoder.next_bits(2);
        if btype == 0 {
            read_uncompressed(&mut decoder, &mut output)?;
        } else if btype == 1 {
            decompress_huffman_static(&mut decoder, &mut output)?;
        } else if btype == 2 {
            decompress_huffman_dynamic(&mut decoder, &mut output)?;
        } else {
            return Err(DecompressResult::IllegalBlockFormat);
        }
        if bfinal != 0 {
            break;
        }
    }
    Ok(output)
}

#[derive(Debug)]
pub enum DecompressResult {
    IllegalBlockFormat,
    UncompressedLengthMismatch,
    TreeError,
    IllegalSmallTree,
}

/// Read the block as is
fn read_uncompressed(decoder: &mut Decoder, output: &mut Vec<u8>) -> Result<(), DecompressResult> {
    let len: u16 = decoder.next_bytes_as_number(2) as u16;
    let nlen: u16 = decoder.next_bytes_as_number(2) as u16;
    let nlen_one_complement = !nlen;
    if len != nlen_one_complement {
        println!("{}, {len}, {nlen_one_complement}", decoder.byte);
        return Err(DecompressResult::UncompressedLengthMismatch);
    }
    output.extend(decoder.next_bytes_as_slice(len.into()));
    for _ in 0..len {
        output.push(decoder.next_byte());
    }
    Ok(())
}

/// Read the block using the static huffman tree (provided by the spec)
fn decompress_huffman_static(
    decoder: &mut Decoder,
    output: &mut Vec<u8>,
) -> Result<(), DecompressResult> {
    // TODO: should be precomputed
    let literal_tree = HuffmanTree::from_bitlengths(
        (0..288)
            .map(|i| {
                if i < 144 {
                    8
                } else if i < 256 {
                    9
                } else if i < 280 {
                    7
                } else {
                    8
                }
            })
            .collect::<Vec<u16>>()
            .as_slice(),
        (0..288).collect::<Vec<u16>>().as_slice(),
    );
    let distance_tree =
        HuffmanTree::from_bitlengths(&[5; 30], (0..30).collect::<Vec<u16>>().as_slice());
    decode_length_distance_pairs(decoder, &literal_tree, &distance_tree, output)
}

/// Read the block using the trees at the beginning of the block
fn decompress_huffman_dynamic(
    decoder: &mut Decoder,
    output: &mut Vec<u8>,
) -> Result<(), DecompressResult> {
    let (literal_tree, distance_tree) = read_trees(decoder)?;
    decode_length_distance_pairs(decoder, &literal_tree, &distance_tree, output)
}

/// Decode the huffman trees at the beginning of the block
/// The two trees are also encoded using a small static tree of max 19 nodes
fn read_trees(decoder: &mut Decoder) -> Result<(HuffmanTree, HuffmanTree), DecompressResult> {
    let hlit = decoder.next_bits(5) + 257;
    let hdist = decoder.next_bits(5) + 1;
    let hclen = decoder.next_bits(4) + 4;

    // Read code lengths for the code length alphabet
    let mut code_length_tree_bitlengths: [u16; 19] = [0; 19];
    for i in 0..hclen {
        code_length_tree_bitlengths[TABLE_CODE_LENGTH_ORDER[i as usize] as usize] =
            decoder.next_bits(3) as u16;
    }
    let code_length_tree = HuffmanTree::from_bitlengths(
        &code_length_tree_bitlengths,
        (0..19).collect::<Vec<u16>>().as_slice(),
    );

    let mut bitlengths: [u16; 1024] = [0; 1024];
    let mut bitlengths_count = 0;
    while (bitlengths_count as u32) < hlit + hdist {
        let symbol = code_length_tree.decode_symbol(decoder)?;
        if symbol <= 15 {
            bitlengths[bitlengths_count] = symbol;
            bitlengths_count += 1;
        } else if symbol == 16 {
            let prev = bitlengths[bitlengths_count - 1];
            let repeat_length = decoder.next_bits(2) + 3;
            for _ in 0..repeat_length {
                bitlengths[bitlengths_count] = prev;
                bitlengths_count += 1;
            }
        } else if symbol == 17 {
            let repeat_length = decoder.next_bits(3) + 3;
            bitlengths_count += repeat_length as usize;
        } else if symbol == 18 {
            let repeat_length = decoder.next_bits(7) + 11;
            bitlengths_count += repeat_length as usize;
        } else {
            return Err(DecompressResult::IllegalSmallTree);
        }
    }

    let literal_tree = HuffmanTree::from_bitlengths(
        &bitlengths[0..(hlit as usize)],
        (0..286).collect::<Vec<u16>>().as_slice(),
    );
    let distance_tree = HuffmanTree::from_bitlengths(
        &bitlengths[(hlit as usize)..(hlit as usize + 30)],
        (0..30).collect::<Vec<u16>>().as_slice(),
    );

    Ok((literal_tree, distance_tree))
}

/// Read the block using the passed trees
fn decode_length_distance_pairs(
    decoder: &mut Decoder,
    literal_tree: &HuffmanTree,
    distance_tree: &HuffmanTree,
    output: &mut Vec<u8>,
) -> Result<(), DecompressResult> {
    for _ in 0..LARGE_ITERATION_COUNT {
        let symbol = literal_tree.decode_symbol(decoder)?;
        if symbol <= 255 {
            output.push(symbol as u8);
        } else if symbol == 256 {
            break;
        } else {
            let special_symbol = (symbol - 257) as usize;
            let length = decoder.next_bits(TABLE_LENGTH_EXTRA_BITS[special_symbol])
                + TABLE_LENGTH_BASE[special_symbol];
            let distance_symbol = distance_tree.decode_symbol(decoder)? as usize;
            let distance = decoder.next_bits(TABLE_DISTANCE_EXTRA_BITS[distance_symbol])
                + TABLE_DISTANCE_BASE[distance_symbol];
            for _ in 0..length {
                let byte = output[output.len() - distance as usize];
                output.push(byte);
            }
        }
    }
    Ok(())
}

/// Tree that encodes a way to parse symbols of variable lengths univocally
#[derive(Clone, Copy)]
struct HuffmanTree {
    nodes: [HuffmanNode; 1024],
    next_free_node: usize,
}

/// Node of the tree. `left` and `right` are offsets in the tree's `nodes` array
#[derive(Clone, Copy, Default)]
struct HuffmanNode {
    symbol: Option<u16>,
    left: Option<usize>,
    right: Option<usize>,
}

impl HuffmanTree {
    /// Create a new empty tree
    fn new() -> Self {
        Self {
            nodes: [HuffmanNode::default(); 1024],
            next_free_node: 1,
        }
    }

    /// Create a tree from a compressed representation described in the inflate spec
    fn from_bitlengths(bitlengths: &[u16], alphabet: &[u16]) -> HuffmanTree {
        // TODO: error handling
        let max_bits = bitlengths.iter().max().unwrap();
        let mut counts = [0; 16];
        let counts_len = max_bits + 1;
        for i in 0..counts_len as u32 {
            counts[i as usize] = bitlengths
                .iter()
                .filter(|&&j| j as u32 == i && j != 0)
                .count() as u32;
        }
        let mut next_code = [0; 1024];
        let mut next_code_len = 2;
        for bits in 2..(max_bits + 1) as usize {
            let next = (next_code[bits - 1] + counts[bits - 1]) << 1;
            next_code[next_code_len] = next;
            next_code_len += 1;
        }
        let mut tree = HuffmanTree::new();
        for i in 0..(alphabet.len().min(bitlengths.len())) {
            if bitlengths[i] != 0 {
                tree.insert(
                    next_code[bitlengths[i] as usize].into(),
                    bitlengths[i].into(),
                    alphabet[i],
                );
                next_code[bitlengths[i] as usize] += 1;
            }
        }
        tree
    }

    /// Add a node to the tree and link it to a previous node
    fn insert(&mut self, code: u32, n: i32, symbol: u16) {
        let mut current = 0;
        for i in (0..n).rev() {
            let bit = code & (1 << i);
            current = if bit != 0 {
                if self.nodes[current].right.is_none() {
                    self.nodes[current].right = Some(self.next_free_node);
                    self.next_free_node += 1;
                }
                self.nodes[current].right.unwrap()
            } else {
                if self.nodes[current].left.is_none() {
                    self.nodes[current].left = Some(self.next_free_node);
                    self.next_free_node += 1;
                }
                self.nodes[current].left.unwrap()
            };
        }
        self.nodes[current].symbol = Some(symbol);
    }

    /// Use the tree to decode a symbol form a bit stream
    fn decode_symbol(&self, decoder: &mut Decoder) -> Result<u16, DecompressResult> {
        let mut current = 0;
        while self.nodes[current].right.is_some() || self.nodes[current].left.is_some() {
            let bit = decoder.next_bit();
            current = if bit != 0 {
                self.nodes[current]
                    .right
                    .ok_or(DecompressResult::TreeError)?
            } else {
                self.nodes[current]
                    .left
                    .ok_or(DecompressResult::TreeError)?
            };
        }
        self.nodes[current]
            .symbol
            .ok_or(DecompressResult::TreeError)
    }
}

/// Constants defined in the spec
const TABLE_CODE_LENGTH_ORDER: [u16; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];
const TABLE_LENGTH_EXTRA_BITS: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const TABLE_LENGTH_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const TABLE_DISTANCE_EXTRA_BITS: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const TABLE_DISTANCE_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Avoiding infinite loops
const LARGE_ITERATION_COUNT: u64 = 100000000;

/// Reads bits and bytes of a data slice in a zlib conformant way
/// The bytes are read in normal order
/// The bits are read in reverse order with respect to the current byte
/// Example: 11101001 -> reads: 1, 0, 0, 1, 0, 1, 1, 1
pub struct Decoder<'a> {
    /// The data to read
    data: &'a [u8],
    /// The current byte
    byte: usize,
    /// The current bit of the current byte
    bit: u8,
}

impl<'a> Decoder<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Self {
        Self {
            data,
            byte: 0,
            bit: 0,
        }
    }

    /// Returns the next byte skipping any unread bits
    pub fn next_bytes_as_slice(&mut self, n: usize) -> &[u8] {
        if self.bit > 0 {
            self.byte += 1;
            self.bit = 0;
        }
        let current = self.byte;
        self.byte += n;
        &self.data[current..self.byte]
    }

    /// Returns the next byte skipping any unread bits
    pub fn next_byte(&mut self) -> u8 {
        if self.bit > 0 {
            self.byte += 1;
            self.bit = 0;
        }
        let byte = self.data[self.byte];
        self.byte += 1;
        byte
    }

    // Returns the next n bytes discarding any unread bits as a number
    pub fn next_bytes_as_number(&mut self, n: u32) -> u32 {
        let mut o: u32 = 0;
        for i in 0..n {
            o |= (self.next_byte() as u32) << (8 * i);
        }
        o
    }

    /// Returns the next bit
    pub fn next_bit(&mut self) -> u8 {
        let byte = self.data[self.byte];
        let bit = (byte >> self.bit) & 1;
        self.bit += 1;
        if self.bit >= 8 {
            self.bit = 0;
            self.byte += 1;
        }
        bit
    }

    /// Returns the next n bits and interprets them as an unsigned integer
    /// The bits are read from least significant to most significant
    /// Example: next_bits(3) of 11001011 -> 011, which is 3
    pub fn next_bits(&mut self, n: u32) -> u32 {
        let mut o: u32 = 0;
        for i in 0..n {
            let bit = self.next_bit() as u32;
            o |= bit << i;
        }
        o
    }
}
