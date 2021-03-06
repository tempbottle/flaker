// Copyright (c) 2016 - Jeremiah Peschka
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
extern crate num;
extern crate byteorder;
use std::time;
use self::num::BigUint;
use self::byteorder::{LittleEndian, WriteBytesExt};

#[derive(Debug)]
pub enum FlakeError {
    ClockIsRunningBackwards
}

pub struct Flaker {
    identifier: [u8; 6],
    last_generated_time_ms: u64,
    counter: u16,
}

#[derive(PartialEq)]
pub enum Endianness {
    LittleEndian,
    BigEndian,
}

impl Flaker {
    /// Returns a new Flaker based on the specified identifier
    ///
    /// # Arguments
    ///
    /// * `identifier` - A 6 byte vec that provides some arbitrary identification.
    ///
    /// # Remarks
    ///
    /// This is a convenience function that converts the `identifier` `vec` into
    /// a 6 byte array. Where possible, prefer the array and use `new`.
    ///
    /// *Note*: This also assumes the `flaker` is being created on a little endian
    /// CPU. 
    pub fn new_from_identifier(identifier: Vec<u8>) -> Flaker {
        let mut a_identifier: [u8; 6] = [0 as u8; 6];
        a_identifier.clone_from_slice(&identifier);
        Flaker::new(a_identifier, Endianness::LittleEndian)
    }

    /// Returns a new Flaker based on the specified identifier
    ///
    /// # Arguments
    ///
    /// * `identifier` - A 6 byte vec that provides some arbitrary identification.
    /// * `little_endian` - For specifying endianness. This is important for byte order when constructing the flake.
    pub fn new(mut identifier: [u8; 6], endian: Endianness) -> Flaker {
        if identifier.len() < 6 {
            panic!("Identifier must have a length of 6");
        }
        
        if endian == Endianness::BigEndian {
            identifier.reverse();
        }

        Flaker { identifier: identifier,
                last_generated_time_ms: Flaker::current_time_in_ms(),
                counter: 0
                }
    }

    /// Returns the current UNIX time in milliseconds
    fn current_time_in_ms() -> u64 {
        let now_ts = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(dur) => dur,
            Err(err) => err.duration(),
        };
        
        // Convert current time to milliseconds by multiplying seconds by 1000
        // Convert current fractional seconds from nanoseconds to milliseconds
        // Then, get the current time as milliseconds.
        now_ts.as_secs() * 1000 + (now_ts.subsec_nanos() / 1000_000) as u64
    }
    
    /// Creates a new flake ID from the identifier, current time, and an internal counter.
    /// Identifiers are generated as 128-bit numbers:
    /// * 64-bit timestamp as milliseconds since the dawn of time (January 1, 1970)
    /// * 48-bit worker identifier
    /// * 16-bit sequence number that is incremented when more than one identifier is requested in the same millisecond and reset to 0 when the clock moves forward
    fn construct_id(&mut self) -> BigUint {
        // Create a new slice of bytes
        let mut bytes = [0 as u8; 16];

        // push the counter into bytes
        bytes[0] = self.counter as u8;
        bytes[1] = (self.counter >> 8) as u8;

        // next 6 bytes are the worker id
        for (pos, byte) in self.identifier.iter().enumerate() {
            bytes[pos + 2] = *byte;
        }

        let mut wtr = vec![];

        wtr.write_u64::<LittleEndian>(self.last_generated_time_ms).unwrap();

        // fill the rest of the buffer with the current time, as bytes
        for (pos, w) in wtr.into_iter().enumerate() {
            bytes[pos + 8] = w;
        }
        
        // create a BigUint from the buffer
        BigUint::from_bytes_le(&bytes)
    }

    /// Update internal data structures.
    fn update(&mut self) -> Result<(), FlakeError> {
        let current_time_in_ms = Flaker::current_time_in_ms();

        if self.last_generated_time_ms > current_time_in_ms {
            return Result::Err(FlakeError::ClockIsRunningBackwards);
        }

        if self.last_generated_time_ms < current_time_in_ms {
            self.counter = 0;
        }
        else {
            self.counter += 1;
        }

        self.last_generated_time_ms = current_time_in_ms;

        Ok(())
    }

    /// Generate a new ID 
    pub fn get_id(&mut self) -> Result<BigUint, FlakeError> {
        self.update().map(|_| self.construct_id())
    }
}

#[test]
fn ids_change_over_time() {
    use std::time::Duration;
    use std::thread;
    
    let mut f1 = Flaker::new_from_identifier(vec![0, 1, 2, 3, 4, 5]);
    let id1 = f1.get_id().unwrap();
    thread::sleep(Duration::from_millis(50));
    let id2 = f1.get_id().unwrap();

    println!("{} < {}", id1, id2);

    assert!(id1 < id2);
}

#[test]
fn ids_change_quickly() {
    let mut f1 = Flaker::new([0, 1, 2, 3, 4, 5], Endianness::LittleEndian);

    let id3 = f1.get_id().unwrap();
    let id4 = f1.get_id().unwrap();
    
    println!("{} < {}", id3, id4);

    assert!(id3 < id4);
}