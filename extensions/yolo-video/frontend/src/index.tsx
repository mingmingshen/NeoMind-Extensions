/**
 * YOLO Video Display Dashboard Component - Optimized Version
 *
 * Zero-processing frontend component that displays MJPEG stream
 * from the backend extension.
 *
 * Features:
 * - Direct <img> display of MJPEG stream
 * - No canvas processing or base64 encoding
 * - Supports Camera, RTSP, and File sources
 * - Real-time statistics from backend
 */

import { useState, useEffect, useRef, useCallback } from 'react'

export interface DataSource {
  type: string
  extensionId?: string
  [key: string]: any
}

export interface YoloVideoDisplayProps {
  title?: string
  dataSource?: DataSource
  className?: string
  // Video source URL (supports multiple protocols)
  sourceUrl?: string
  // Legacy support
  videoSource?: 'camera' | 'file' | 'rtsp'
  rtspUrl?: string
  confidenceThreshold?: number
  maxObjects?: number
  fps?: number
  drawBoxes?: boolean
  showStats?: boolean
  variant?: 'default' | 'compact'
}

interface StreamInfo {
  stream_id: string
  stream_url: string
  status: 'Starting' | 'Running' | 'Stopped' | string
  source_type: string
  width: number
  height: number
  fps: number
}

interface StreamStats {
  frame_count: number
  fps: number
  total_detections: number
  detected_objects: Record<string, number>
}

const VideoIcon = ({ className = "w-5 h-5" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 10.5l4.72-4.72a.75.75 0 011.28.53v11.38a.75.75 0 01-1.28.53l-4.72-4.72M4.5 18.75h9a2.25 2.25 0 002.25-2.25v-9a2.25 2.25 0 00-2.25-2.25h-9A2.25 2.25 0 002.25 7.5v9a2.25 2.25 0 002.25 2.25z" />
  </svg>
)

const PlayIcon = ({ className = "w-5 h-5" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.971l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z" />
  </svg>
)

const StopIcon = ({ className = "w-5 h-5" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M5.25 7.5A2.25 2.25 0 017.5 5.25h9a2.25 2.25 0 012.25 2.25v9a2.25 2.25 0 01-2.25 2.25h-9a2.25 2.25 0 01-2.25-2.25v-9z" />
  </svg>
)

const TargetIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M15 10.5a3 3 0 11-6 0 3 3 0 016 0z" />
    <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 10.5c0 7.142-7.5 11.25-7.5 11.25S4.5 17.642 4.5 10.5a7.5 7.5 0 1115 0z" />
  </svg>
)

const SpeedIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z" />
  </svg>
)

const ClockIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
  </svg>
)

const EXTENSION_ID = 'yolo-video'

export const YoloVideoDisplay = function YoloVideoDisplay({
  title = 'YOLO Video',
  dataSource,
  className = '',
  sourceUrl,
  videoSource = 'camera',
  rtspUrl = '',
  confidenceThreshold = 0.5,
  maxObjects = 20,
  fps = 15,
  drawBoxes = true,
  showStats = true,
  variant = 'default'
}: YoloVideoDisplayProps) {
  const [isRunning, setIsRunning] = useState(false)
  const [streamInfo, setStreamInfo] = useState<StreamInfo | null>(null)
  const [stats, setStats] = useState<StreamStats | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [currentSource, setCurrentSource] = useState(videoSource)
  const [sourceUrlInput, setSourceUrlInput] = useState(sourceUrl || `camera://0`)
  const [sessionTime, setSessionTime] = useState(0)
  const statsIntervalRef = useRef<NodeJS.Timeout | null>(null)
  const sessionTimerRef = useRef<NodeJS.Timeout | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const isCompact = variant === 'compact'

  // Fetch stream statistics
  const fetchStats = useCallback(async (streamId: string) => {
    try {
      const response = await fetch(`/api/extensions/${EXTENSION_ID}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command: 'get_stream_stats',
          args: { stream_id: streamId }
        })
      })

      if (response.ok) {
        const data = await response.json()
        if (data.success && data.data) {
          setStats(data.data)
        }
      }
    } catch (e) {
      console.error('Failed to fetch stats:', e)
    }
  }, [])

  // Build source URL from props
  const getSourceUrl = useCallback(() => {
    // Use new sourceUrl if provided
    if (sourceUrl) {
      return sourceUrl
    }

    // Use input URL
    if (sourceUrlInput && sourceUrlInput !== '') {
      return sourceUrlInput
    }

    // Legacy support - build URL from videoSource and rtspUrl
    switch (currentSource) {
      case 'camera':
        return 'camera://0'
      case 'rtsp':
        return rtspUrl && rtspUrl.startsWith('rtsp://') ? rtspUrl : `rtsp://${rtspUrl || '192.168.1.100/stream'}`
      case 'file':
        return 'file:///path/to/video.mp4'
      default:
        return 'camera://0'
    }
  }, [sourceUrl, sourceUrlInput, currentSource, rtspUrl])

  // Start stream
  const startStream = useCallback(async () => {
    try {
      setError(null)
      setStats(null)

      const finalSourceUrl = getSourceUrl()
      if (!finalSourceUrl) {
        setError('Please specify a valid video source')
        return
      }

      const response = await fetch(`/api/extensions/${EXTENSION_ID}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command: 'start_stream',
          args: {
            source_url: finalSourceUrl,
            confidence_threshold: confidenceThreshold,
            max_objects: maxObjects,
            target_fps: fps,
            draw_boxes: drawBoxes
          }
        })
      })

      if (response.ok) {
        const data = await response.json()
        if (data.success && data.data) {
          setStreamInfo(data.data)
          setIsRunning(true)
          setSessionTime(0)

          // Start session timer
          sessionTimerRef.current = setInterval(() => {
            setSessionTime(t => t + 1)
          }, 1000)

          // Start stats polling
          if (showStats) {
            statsIntervalRef.current = setInterval(() => {
              fetchStats(data.data.stream_id)
            }, 2000)
          }
        } else {
          setError(data.error || 'Failed to start stream')
        }
      } else {
        setError('Failed to communicate with extension')
      }
    } catch (e) {
      setError('Network error: ' + (e as Error).message)
    }
  }, [getSourceUrl, confidenceThreshold, maxObjects, fps, drawBoxes, showStats, fetchStats])

  // Stop stream
  const stopStream = useCallback(async () => {
    if (!streamInfo) return

    try {
      await fetch(`/api/extensions/${EXTENSION_ID}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command: 'stop_stream',
          args: { stream_id: streamInfo.stream_id }
        })
      })
    } catch (e) {
      console.error('Failed to stop stream:', e)
    } finally {
      setIsRunning(false)
      setStreamInfo(null)
      setStats(null)

      if (statsIntervalRef.current) {
        clearInterval(statsIntervalRef.current)
        statsIntervalRef.current = null
      }
      if (sessionTimerRef.current) {
        clearInterval(sessionTimerRef.current)
        sessionTimerRef.current = null
      }
    }
  }, [streamInfo])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (isRunning) {
        stopStream()
      }
    }
  }, [isRunning, stopStream])

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60)
    const secs = seconds % 60
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }

  // Get MJPEG stream URL
  const getStreamUrl = () => {
    if (!streamInfo) return ''
    // Stream URL is relative to API base
    return streamInfo.stream_url.startsWith('/') ? streamInfo.stream_url : `/${streamInfo.stream_url}`
  }

  // Get detection count
  const detectedCount = stats?.total_detections || 0
  const avgFps = stats?.fps || streamInfo?.fps || 0
  const totalFrames = stats?.frame_count || 0

  // Get detected objects summary
  const objectSummary = stats?.detected_objects
    ? Object.entries(stats.detected_objects)
        .sort(([, a], [, b]) => b - a)
        .slice(0, 5)
        .map(([name, count]) => `${name}: ${count}`)
        .join(', ')
    : ''

  return (
    <div
      ref={containerRef}
      className={`relative overflow-hidden bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900 rounded-xl shadow-2xl transition-all duration-300 ease-out ${className}`}
      style={{ minHeight: isCompact ? '200px' : '300px' }}
    >
      {/* Background decorations */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute -top-20 -right-20 w-40 h-40 bg-emerald-500/10 rounded-full blur-3xl" />
        <div className="absolute -bottom-20 -left-20 w-32 h-32 bg-cyan-500/10 rounded-full blur-2xl" />
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-64 h-64 bg-purple-500/5 rounded-full blur-3xl" />
      </div>

      <div className={`relative ${isCompact ? 'p-3' : 'p-4 sm:p-5'}`}>
        {/* Header */}
        {showStats && (
          <div className={`flex items-center justify-between ${isCompact ? 'mb-2' : 'mb-4'}`}>
            <div className="flex items-center gap-2">
              <div className={`bg-emerald-500/20 backdrop-blur-sm rounded-lg border border-emerald-500/30 ${isCompact ? 'p-1' : 'p-1.5'}`}>
                <VideoIcon className={`${isCompact ? 'w-3.5 h-3.5' : 'w-4 h-4'} text-emerald-400`} />
              </div>
              <div>
                <h3 className={`font-semibold text-white ${isCompact ? 'text-xs' : 'text-sm sm:text-base'}`}>
                  {title}
                </h3>
                {!isCompact && (
                  <p className="text-white/40 text-[10px] capitalize">
                    {currentSource} Stream • {streamInfo?.status || 'Ready'}
                  </p>
                )}
              </div>
            </div>

            {/* Stats */}
            <div className="flex items-center gap-1.5 sm:gap-3">
              {isRunning && !isCompact && (
                <div className="flex items-center gap-1.5 bg-emerald-500/20 backdrop-blur-sm rounded-lg px-2 py-1 border border-emerald-500/30">
                  <div className="w-2 h-2 bg-emerald-400 rounded-full animate-pulse" />
                  <span className="text-emerald-300 text-xs font-mono">{formatTime(sessionTime)}</span>
                </div>
              )}
              <div className="flex items-center gap-1.5 bg-white/5 backdrop-blur-sm rounded-lg px-2 py-1 border border-white/10">
                <TargetIcon className="w-3.5 h-3.5 text-emerald-400" />
                <span className={`font-semibold text-white ${isCompact ? 'text-xs' : 'text-sm'}`}>{detectedCount}</span>
              </div>
              {!isCompact && (
                <div className="hidden sm:flex items-center gap-1.5 bg-white/5 backdrop-blur-sm rounded-lg px-2 py-1 border border-white/10">
                  <SpeedIcon className="w-3.5 h-3.5 text-amber-400" />
                  <span className="text-white/70 text-xs">{Math.round(avgFps)} FPS</span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Video Display */}
        <div
          className={`relative bg-black/50 rounded-xl overflow-hidden border border-white/10 ${isCompact ? 'aspect-video' : 'aspect-video sm:aspect-[4/3]'}`}
        >
          {/* MJPEG Stream Image */}
          {isRunning && streamInfo ? (
            <img
              src={getStreamUrl()}
              alt="YOLO Detection Stream"
              className="w-full h-full object-contain"
              onError={() => setError('Stream connection lost')}
            />
          ) : (
            /* Not Running Placeholder */
            <div className="absolute inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center">
              <div className="text-center">
                <VideoIcon className={`${isCompact ? 'w-8 h-8' : 'w-12 h-12'} text-white/30 mx-auto mb-2`} />
                <p className={`text-white/50 ${isCompact ? 'text-xs' : 'text-sm'}`}>
                  Click Start to begin detection
                </p>
              </div>
            </div>
          )}

          {/* Live Indicator */}
          {isRunning && (
            <div className="absolute top-2 right-2 bg-emerald-500/80 backdrop-blur-sm rounded-full px-2 py-0.5">
              <div className="flex items-center gap-1">
                <div className="w-1.5 h-1.5 bg-white rounded-full animate-pulse" />
                <span className="text-white text-[9px] font-medium">LIVE</span>
              </div>
            </div>
          )}
        </div>

        {/* Controls */}
        <div className={`flex items-center gap-2 mt-3 ${isCompact ? 'flex-wrap' : ''}`}>
          {/* Protocol Preset Buttons */}
          <div className="flex gap-1">
            {[
              { key: 'camera', label: 'Camera', url: 'camera://0' },
              { key: 'rtsp', label: 'RTSP', url: '' },
              { key: 'hls', label: 'HLS', url: '' },
              { key: 'rtmp', label: 'RTMP', url: '' },
            ].map(({ key, label, url }) => (
              <button
                key={key}
                onClick={() => {
                  if (isRunning) stopStream()
                  setCurrentSource(key as any)
                  if (url) setSourceUrlInput(url)
                }}
                className={`px-2 py-1 rounded text-xs font-medium transition-colors ${
                  (currentSource === key || (sourceUrl?.startsWith(key)))
                    ? 'bg-blue-500 text-white'
                    : 'bg-white/10 text-white/70 hover:bg-white/20'
                }`}
              >
                {label}
              </button>
            ))}
          </div>

          {/* URL Input */}
          <input
            type="text"
            value={sourceUrl || getSourceUrl()}
            onChange={(e) => setSourceUrlInput(e.target.value)}
            placeholder="camera://0 or rtsp://192.168.1.100/stream"
            className={`flex-1 bg-white/10 backdrop-blur-sm border border-white/20 rounded-lg text-white text-xs focus:outline-none focus:border-blue-400 ${isCompact ? 'px-2 py-1' : 'px-2.5 py-1.5'}`}
          />

          {error && (
            <span className="text-red-400 text-[10px] sm:text-xs truncate max-w-[150px]">{error}</span>
          )}

          <button
            onClick={isRunning ? stopStream : startStream}
            className={`flex items-center gap-1.5 ${isCompact ? 'px-2.5 py-1' : 'px-3 sm:px-4 py-1.5'} rounded-lg text-xs sm:text-sm font-semibold transition-all duration-200 ${
              isRunning
                ? 'bg-red-500 hover:bg-red-600 text-white shadow-lg shadow-red-500/25'
                : 'bg-emerald-500 hover:bg-emerald-600 text-white shadow-lg shadow-emerald-500/25'
            }`}
          >
            {isRunning ? (
              <>
                <StopIcon className={`${isCompact ? 'w-3 h-3' : 'w-4 h-4'}`} />
                <span>Stop</span>
              </>
            ) : (
              <>
                <PlayIcon className={`${isCompact ? 'w-3 h-3' : 'w-4 h-4'}`} />
                <span>Start</span>
              </>
            )}
          </button>
        </div>

        {/* Stats Bar */}
        {showStats && stats && !isCompact && (
          <>
            <div className="flex items-center gap-4 mt-3 pt-3 border-t border-white/10">
              <div className="flex items-center gap-1.5">
                <TargetIcon className="w-4 h-4 text-white/40" />
                <span className="text-white/60 text-xs">
                  Frames: <span className="text-white font-medium">{totalFrames.toLocaleString()}</span>
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <SpeedIcon className="w-4 h-4 text-white/40" />
                <span className="text-white/60 text-xs">
                  Avg FPS: <span className="text-white font-medium">{Math.round(avgFps)}</span>
                </span>
              </div>
              {isRunning && (
                <div className="flex items-center gap-1.5">
                  <ClockIcon className="w-4 h-4 text-white/40" />
                  <span className="text-white/60 text-xs">
                    Time: <span className="text-white font-medium font-mono">{formatTime(sessionTime)}</span>
                  </span>
                </div>
              )}
            </div>

            {/* Detected Objects Summary */}
            {objectSummary && (
              <div className="mt-2 text-xs text-white/60 truncate">
                {objectSummary}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}

YoloVideoDisplay.displayName = 'YoloVideoDisplay'
export default { YoloVideoDisplay }
