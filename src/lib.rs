use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let v = self.value;
        if v <= 0xFC {
            vec![v as u8]
        } else if v <= 0xFFFF {
            let mut bytes = vec![0xFD];
            bytes.extend_from_slice(&(v as u16).to_le_bytes());
            bytes
        } else if v <= 0xFFFF_FFFF {
            let mut bytes = vec![0xFE];
            bytes.extend_from_slice(&(v as u32).to_le_bytes());
            bytes
        } else {
            let mut bytes = vec![0xFF];
            bytes.extend_from_slice(&v.to_le_bytes());
            bytes
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }
        match bytes[0] {
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
                Ok((CompactSize { value }, 9))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as u64;
                Ok((CompactSize { value }, 5))
            }
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u16::from_le_bytes(bytes[1..3].try_into().unwrap()) as u64;
                Ok((CompactSize { value }, 3))
            }
            b => Ok((CompactSize { value: b as u64 }, 1)),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex_str = hex::encode(self.0);
        serializer.serialize_str(&hex_str)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        let decoded = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        if decoded.len() != 32 {
            return Err(serde::de::Error::custom("Txid must be exactly 32 bytes"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&decoded);
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        OutPoint {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        bytes.extend_from_slice(&self.txid.0);
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        Ok((
            OutPoint {
                txid: Txid(txid),
                vout,
            },
            36,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let len = CompactSize::new(self.bytes.len() as u64);
        let mut out = len.to_bytes();
        out.extend_from_slice(&self.bytes);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len, prefix_size) = CompactSize::from_bytes(bytes)?;
        let script_len = len.value as usize;
        let total = prefix_size + script_len;
        if bytes.len() < total {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[prefix_size..total].to_vec();
        Ok((
            Script {
                bytes: script_bytes,
            },
            total,
        ))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        TransactionInput {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.previous_output.to_bytes());
        bytes.extend(self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut offset = 0;

        let (previous_output, consumed) = OutPoint::from_bytes(&bytes[offset..])?;
        offset += consumed;

        let (script_sig, consumed) = Script::from_bytes(&bytes[offset..])?;
        offset += consumed;

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;

        Ok((
            TransactionInput {
                previous_output,
                script_sig,
                sequence,
            },
            offset,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            bytes.extend(input.to_bytes());
        }
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut offset = 0;

        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        offset += 4;

        let (input_count, consumed) = CompactSize::from_bytes(&bytes[offset..])?;
        offset += consumed;

        let mut inputs = Vec::new();
        for _ in 0..input_count.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[offset..])?;
            offset += consumed;
            inputs.push(input);
        }

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;

        Ok((
            BitcoinTransaction {
                version,
                inputs,
                lock_time,
            },
            offset,
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Inputs: {}", self.inputs.len())?;
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "  Input {}:", i)?;
            writeln!(
                f,
                "    Previous Output Txid: {}",
                hex::encode(input.previous_output.txid.0)
            )?;
            writeln!(
                f,
                "    Previous Output Vout: {}",
                input.previous_output.vout
            )?;
            writeln!(f, "    ScriptSig Length: {}", input.script_sig.bytes.len())?;
            writeln!(f, "    ScriptSig Bytes: {:?}", input.script_sig.bytes)?;
            writeln!(f, "    Sequence: {}", input.sequence)?;
        }
        write!(f, "Lock Time: {}", self.lock_time)
    }
}
