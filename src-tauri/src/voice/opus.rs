//! Opus 音频编解码器封装
//! 
//! 使用 rusty-opus (纯 Rust 实现) 提供 Opus 编码/解码功能
//! 无需 cmake，纯 Rust 实现，适合跨平台编译

use rusty_opus::{Encoder, Decoder, Application, Channels, Bitrate, SignalType};
use std::sync::Arc;
use std::collections::VecDeque;

/// Opus 编码器封装
pub struct OpusEncoder {
    encoder: Encoder,
    sample_rate: u32,
    channels: u16,
    frame_size: usize,
}

impl OpusEncoder {
    /// 创建新的 Opus 编码器
    pub fn new(sample_rate: u32, channels: u16, frame_size: usize) -> Result<Self, String> {
        let channels = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(format!("不支持的声道数: {}", channels)),
        };

        let encoder = Encoder::new(
            sample_rate,
            channels,
            Application::Voip,
        ).map_err(|e| format!("创建 Opus 编码器失败: {:?}", e))?;

        // 设置比特率
        encoder.set_bitrate(64000).map_err(|e| format!("设置比特率失败: {:?}", e))?;

        // 设置复杂度 (0-10, 越高质量越好但 CPU 越高)
        encoder.set_complexity(10).map_err(|e| format!("设置复杂度失败: {:?}", e))?;

        // 设置信号类型为语音
        encoder.set_signal(SignalType::Voice)
            .map_err(|e| format!("设置信号类型失败: {:?}", e))?;

        // 启用 DTX (不传输静音)
        encoder.set_dtx(true).map_err(|e| format!("启用 DTX 失败: {:?}", e))?;

        Ok(Self {
            encoder,
            sample_rate,
            channels: 1, // 当前只支持单声道
            frame_size,
        })
    }

    /// 编码音频数据
    ///
    /// 输入: PCM 16-bit 样本数据 (i16)
    /// 输出: Opus 编码后的字节数据
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, String> {
        let expected_len = self.frame_size as usize * self.channels as usize;
        if pcm.len() != expected_len {
            return Err(format!("输入帧大小不匹配: 期望 {} 样本, 实际 {}", expected_len, pcm.len()));
        }

        let mut output = vec![0u8; 4000]; // 最大 Opus 包大小
        let len = self.encoder.encode(pcm, &mut output)
            .map_err(|e| format!("Opus 编码失败: {:?}", e))?;

        output.truncate(len);
        Ok(output)
    }

    /// 获取帧大小
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }
}

/// Opus 解码器
pub struct OpusDecoder {
    decoder: Decoder,
    sample_rate: u32,
    channels: u16,
    frame_size: usize,
}

impl OpusDecoder {
    /// 创建新的 Opus 解码器
    pub fn new(sample_rate: u32, channels: u16, frame_size: usize) -> Result<Self, String> {
        let channels = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(format!("不支持的声道数: {}", channels)),
        };

        let decoder = Decoder::new(sample_rate, Channels::Mono)
            .map_err(|e| format!("创建 Opus 解码器失败: {:?}", e))?;

        Ok(Self {
            decoder,
            sample_rate,
            channels: 1,
            frame_size,
        })
    }

    /// 解码音频数据
    ///
    /// 输入: Opus 编码字节数据
    /// 输出: PCM 16-bit 样本数据 (i16)
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<i16>, String> {
        let mut output = vec![0i16; self.frame_size as usize];
        let len = self.decoder.decode(data, &mut output, false)
            .map_err(|e| format!("Opus 解码失败: {:?}", e))?;

        output.truncate(len);
        Ok(output)
    }

    /// 获取帧大小
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }
}

/// Opus 编解码器管理器
pub struct OpusCodec {
    encoder: Option<OpusEncoder>,
    decoder: Option<OpusDecoder>,
    config: VoiceConfig,
}

impl OpusCodec {
    /// 创建新的 Opus 编解码器
    pub fn new(config: VoiceConfig) -> Result<Self, String> {
        let encoder = OpusEncoder::new(
            config.sample_rate,
            config.channels,
            config.frame_size,
        )?;
        
        let decoder = OpusDecoder::new(
            config.sample_rate,
            config.channels,
            config.frame_size,
        )?;

        Ok(Self {
            encoder: Some(encoder),
            decoder: Some(decoder),
            config,
        })
    }

    /// 编码音频帧
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, String> {
        self.encoder.as_mut().ok_or("编码器未初始化")?.encode(pcm)
    }

    /// 解码音频帧
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<i16>, String> {
        self.decoder.as_mut().ok_or("解码器未初始化")?.decode(data)
    }

    /// 获取帧大小
    pub fn frame_size(&self) -> usize {
        self.config.frame_size
    }

    /// 获取采样率
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// 获取声道数
    pub fn channels(&self) -> u16 {
        self.config.channels
    }
}

/// 语音配置
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    pub space_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_size: usize,
    pub bitrate: u32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            space_id: String::new(),
            sample_rate: 48000,
            channels: 1,
            frame_size: 960, // 20ms at 48kHz
            bitrate: 64000,
        }
    }
}

/// 音频数据包（用于网络传输）
#[derive(Debug, Clone)]
pub struct AudioPacket {
    /// 音频数据
    pub data: Vec<u8>,
    /// 时间戳 (毫秒)
    pub timestamp: u64,
    /// 序列号
    pub sequence: u32,
    /// 采样率
    pub sample_rate: u32,
    /// 声道数
    pub channels: u16,
    /// 帧大小
    pub frame_size: usize,
}

impl AudioPacket {
    pub fn new(data: Vec<u8>, sample_rate: u32, channels: u16, frame_size: usize) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        
        Self {
            data,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            sequence: SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            sample_rate,
            channels,
            frame_size,
        }
    }
}

/// 音频数据队列（用于发送/接收缓冲）
pub struct AudioQueue {
    send_queue: Arc<tokio::sync::Mutex<VecDeque<AudioPacket>>>,
    recv_queue: Arc<tokio::sync::Mutex<VecDeque<AudioPacket>>>,
    max_size: usize,
}


impl AudioQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            send_queue: Arc::new(tokio::sync::Mutex::new(VecDeque::with_capacity(max_size))),
            recv_queue: Arc::new(tokio::sync::Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// 推送发送数据
    pub async fn push_send(&self, packet: AudioPacket) -> Result<(), String> {
        let mut queue = self.send_queue.lock().await;
        if queue.len() >= self.max_size {
            queue.pop_front(); // 丢弃最旧的包
        }
        queue.push_back(packet);
        Ok(())
    }

    /// 弹出发送数据
    pub async fn pop_send(&self) -> Option<AudioPacket> {
        self.send_queue.lock().await.pop_front()
    }

    /// 推送接收数据
    pub async fn push_recv(&self, packet: AudioPacket) -> Result<(), String> {
        let mut queue = self.recv_queue.lock().await;
        if queue.len() >= self.max_size {
            queue.pop_front();
        }
        queue.push_back(packet);
        Ok(())
    }

    /// 弹出接收数据
    pub async fn pop_recv(&self) -> Option<AudioPacket> {
        self.recv_queue.lock().await.pop_front()
    }

    /// 获取发送队列大小
    pub async fn send_len(&self) -> usize {
        self.send_queue.lock().await.len()
    }

    /// 获取接收队列大小
    pub async fn recv_len(&self) -> usize {
        self.recv_queue.lock().await.len()
    }
}