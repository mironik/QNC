use crate::{Code, Error, Result};
use serde::{Deserialize, Serialize};

/// Device routing only. Source channels keep their identity; no summing or resampling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelMap {
    source_channels: u16,
    output_channels: Vec<u16>,
}
impl ChannelMap {
    /// Preserve each native channel as its own output; no stereo fallback.
    pub fn identity(source_channels: u16) -> Result<Self> {
        Self::new(source_channels, (0..source_channels).collect())
    }
    /// Zero-based source channel for each device channel. Repetition is explicit dual mono.
    pub fn new(source_channels: u16, output_channels: Vec<u16>) -> Result<Self> {
        let map = Self {
            source_channels,
            output_channels,
        };
        map.validate()?;
        Ok(map)
    }
    pub fn validate(&self) -> Result<()> {
        if !(1..=64).contains(&self.source_channels)
            || self.output_channels.is_empty()
            || self.output_channels.len() > 64
            || self
                .output_channels
                .iter()
                .any(|i| *i >= self.source_channels)
        {
            return Err(Error::new(Code::Contract, "invalid device channel map"));
        }
        Ok(())
    }
    pub fn source_channels(&self) -> u16 {
        self.source_channels
    }
    pub fn output_channels(&self) -> &[u16] {
        &self.output_channels
    }
    pub fn route(&self, source: &[f32], output: &mut Vec<f32>) -> Result<()> {
        self.validate()?;
        let channels = usize::from(self.source_channels);
        if !source.len().is_multiple_of(channels) {
            return Err(Error::new(
                Code::Contract,
                "incomplete native audio sample frame",
            ));
        }
        output.clear();
        output.reserve(source.len() / channels * self.output_channels.len());
        for frame in source.chunks_exact(channels) {
            output.extend(self.output_channels.iter().map(|i| frame[usize::from(*i)]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_preserves_two_and_four_discrete_channels_without_crosstalk() {
        for channels in [2, 4] {
            let map = ChannelMap::identity(channels).unwrap();
            for active in 0..channels {
                let mut source = vec![0.0; usize::from(channels) * 3];
                source[usize::from(active)] = 0.75;
                let mut output = Vec::new();
                map.route(&source, &mut output).unwrap();
                assert_eq!(output, source);
            }
        }
        assert!(ChannelMap::identity(0).is_err());
        assert!(ChannelMap::identity(65).is_err());
    }
    #[test]
    fn four_discrete_channels_are_not_a_stereo_downmix() {
        let source = [0.1, 0.2, 0.3, 0.4, -0.1, -0.2, -0.3, -0.4];
        let mut output = Vec::new();
        ChannelMap::new(4, vec![2, 3])
            .unwrap()
            .route(&source, &mut output)
            .unwrap();
        assert_eq!(output, [0.3, 0.4, -0.3, -0.4]);
        ChannelMap::new(4, vec![1, 1])
            .unwrap()
            .route(&source, &mut output)
            .unwrap();
        assert_eq!(output, [0.2, 0.2, -0.2, -0.2]);
        ChannelMap::new(4, vec![0, 1, 2, 3])
            .unwrap()
            .route(&source, &mut output)
            .unwrap();
        assert_eq!(output, source);
    }
    #[test]
    fn invalid_route_or_incomplete_samples_are_rejected() {
        assert!(ChannelMap::new(4, vec![4]).is_err());
        assert!(ChannelMap::new(0, vec![0]).is_err());
        assert!(ChannelMap::new(4, vec![]).is_err());
        assert!(
            ChannelMap::new(4, vec![0, 1])
                .unwrap()
                .route(&[0.0; 3], &mut Vec::new())
                .is_err()
        );
    }
}
