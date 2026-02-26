/**
 * Image Analyzer Dashboard Component
 * Modern responsive design with SVG icons and professional UI
 * Supports adaptive layout for different container sizes
 */

import { forwardRef, useEffect, useState, useRef, useCallback, useMemo } from 'react'

export interface DataSource {
  type: string
  extensionId?: string
  [key: string]: any
}

export interface ImageAnalyzerProps {
  title?: string
  dataSource?: DataSource
  className?: string
  showMetrics?: boolean
  maxImageSize?: number
  confidenceThreshold?: number
  variant?: 'default' | 'compact'
}

interface Detection {
  class_name: string
  confidence: number
  bbox: { x: number; y: number; width: number; height: number }
}

interface AnalysisResult {
  detections: Detection[]
  image_width: number
  image_height: number
  processing_time_ms: number
  description?: string
}

interface MetricsData {
  images_processed?: number
  avg_processing_time_ms?: number
  total_detections?: number
}

const COLORS = [
  '#ef4444', '#f97316', '#eab308', '#22c55e', '#06b6d4',
  '#3b82f6', '#8b5cf6', '#ec4899', '#f43f5e', '#84cc16'
]

const UploadIcon = ({ className = "w-6 h-6" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5" />
  </svg>
)

const ImageIcon = ({ className = "w-5 h-5" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909m-18 3.75h16.5a1.5 1.5 0 001.5-1.5V6a1.5 1.5 0 00-1.5-1.5H3.75A1.5 1.5 0 002.25 6v12a1.5 1.5 0 001.5 1.5zm10.5-11.25h.008v.008h-.008V8.25zm.375 0a.375.375 0 11-.75 0 .375.375 0 01.75 0z" />
  </svg>
)

const TargetIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M15 10.5a3 3 0 11-6 0 3 3 0 016 0z" />
    <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 10.5c0 7.142-7.5 11.25-7.5 11.25S4.5 17.642 4.5 10.5a7.5 7.5 0 1115 0z" />
  </svg>
)

const ClockIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
  </svg>
)

const ClearIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
  </svg>
)

const RefreshIcon = ({ className = "w-4 h-4" }: { className?: string }) => (
  <svg className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
    <path strokeLinecap="round" strokeLinejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" />
  </svg>
)

const EXTENSION_ID = 'image-analyzer'

export const ImageAnalyzer = forwardRef<HTMLDivElement, ImageAnalyzerProps>(
  function ImageAnalyzer(props, ref) {
    const {
      title = 'Image Analyzer',
      dataSource,
      className = '',
      showMetrics = true,
      maxImageSize = 10 * 1024 * 1024,
      confidenceThreshold = 0.5,
      variant = 'default'
    } = props

    const [image, setImage] = useState<string | null>(null)
    const [imageSize, setImageSize] = useState<{ width: number; height: number } | null>(null)
    const [result, setResult] = useState<AnalysisResult | null>(null)
    const [analyzing, setAnalyzing] = useState(false)
    const [error, setError] = useState<string | null>(null)
    const [metrics, setMetrics] = useState<MetricsData>({})
    const [isDragging, setIsDragging] = useState(false)
    const containerRef = useRef<HTMLDivElement>(null)
    const fileInputRef = useRef<HTMLInputElement>(null)

    const fetchMetrics = useCallback(async () => {
      if (!dataSource?.extensionId) return
      try {
        const response = await fetch(`/api/extensions/${dataSource.extensionId}/metrics`)
        if (response.ok) {
          const data = await response.json()
          if (data.success && data.data) setMetrics(data.data)
        }
      } catch { /* ignore */ }
    }, [dataSource?.extensionId])

    const handleImageUpload = useCallback((file: File) => {
      if (file.size > maxImageSize) {
        setError(`File too large (max ${maxImageSize / 1024 / 1024}MB)`)
        return
      }
      if (!file.type.startsWith('image/')) {
        setError('Please upload an image file')
        return
      }
      const reader = new FileReader()
      reader.onload = (e) => {
        const dataUrl = e.target?.result as string
        setImage(dataUrl)
        setResult(null)
        setError(null)
        const img = new Image()
        img.onload = () => setImageSize({ width: img.width, height: img.height })
        img.src = dataUrl
      }
      reader.readAsDataURL(file)
    }, [maxImageSize])

    const blobToBase64 = useCallback((blob: Blob): Promise<string> => {
      return new Promise((resolve) => {
        const reader = new FileReader()
        reader.onloadend = () => resolve(reader.result as string)
        reader.readAsDataURL(blob)
      })
    }, [])

    const analyzeImage = useCallback(async () => {
      if (!image || !dataSource?.extensionId) return
      setAnalyzing(true)
      setError(null)
      try {
        const response = await fetch(image)
        const blob = await response.blob()
        const base64 = await blobToBase64(blob)
        
        const processResponse = await fetch(`/api/extensions/${dataSource.extensionId}/command`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            command: 'analyze_image',
            args: { image: base64.split(',')[1] }
          })
        })
        
        if (processResponse.ok) {
          const data = await processResponse.json()
          if (data.success && data.data) {
            const mappedResult: AnalysisResult = {
              detections: (data.data.objects || []).map((obj: any) => ({
                class_name: obj.label || obj.class_name || 'object',
                confidence: obj.confidence || 0,
                bbox: obj.bbox || { x: 0, y: 0, width: 0, height: 0 }
              })),
              image_width: imageSize?.width || 640,
              image_height: imageSize?.height || 480,
              processing_time_ms: data.data.processing_time_ms || 0,
              description: data.data.description
            }
            setResult(mappedResult)
            fetchMetrics()
          } else {
            setError(data.error || 'Analysis failed')
          }
        } else {
          setError('Processing request failed')
        }
      } catch {
        setError('Connection error')
      } finally {
        setAnalyzing(false)
      }
    }, [image, dataSource?.extensionId, imageSize, blobToBase64, fetchMetrics])

    useEffect(() => {
      fetchMetrics()
      const interval = setInterval(fetchMetrics, 5000)
      return () => clearInterval(interval)
    }, [fetchMetrics])

    const handleDrop = useCallback((e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(false)
      const file = e.dataTransfer.files[0]
      if (file) handleImageUpload(file)
    }, [handleImageUpload])

    const handleDragOver = useCallback((e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(true)
    }, [])

    const handleDragLeave = useCallback((e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(false)
    }, [])

    const filteredDetections = useMemo(() => {
      if (!result) return []
      return result.detections.filter(d => d.confidence >= confidenceThreshold)
    }, [result, confidenceThreshold])

    const renderBoundingBoxes = useCallback(() => {
      if (!result || !imageSize) return null

      return filteredDetections.map((d, i) => {
        const color = COLORS[i % COLORS.length]
        const x = (d.bbox.x / imageSize.width) * 100
        const y = (d.bbox.y / imageSize.height) * 100
        const w = (d.bbox.width / imageSize.width) * 100
        const h = (d.bbox.height / imageSize.height) * 100

        return (
          <div
            key={i}
            className="absolute pointer-events-none"
            style={{
              left: `${x}%`,
              top: `${y}%`,
              width: `${w}%`,
              height: `${h}%`,
              border: `2px solid ${color}`,
              boxShadow: `0 0 4px ${color}40`
            }}
          >
            <div
              className="absolute -top-5 left-0 px-1.5 py-0.5 text-[10px] font-bold text-white rounded whitespace-nowrap"
              style={{ backgroundColor: color }}
            >
              {d.class_name} {Math.round(d.confidence * 100)}%
            </div>
          </div>
        )
      })
    }, [result, imageSize, filteredDetections])

    const isCompact = variant === 'compact'

    return (
      <div
        ref={(node) => {
          (containerRef as any).current = node
          if (typeof ref === 'function') ref(node)
          else if (ref) ref.current = node
        }}
        className={`relative overflow-hidden bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900 rounded-xl shadow-2xl transition-all duration-300 ease-out ${className}`}
        style={{ minHeight: isCompact ? '200px' : '320px' }}
      >
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          <div className="absolute -top-20 -right-20 w-40 h-40 bg-blue-500/10 rounded-full blur-3xl" />
          <div className="absolute -bottom-20 -left-20 w-32 h-32 bg-purple-500/10 rounded-full blur-2xl" />
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-64 h-64 bg-cyan-500/5 rounded-full blur-3xl" />
        </div>

        <div className={`relative ${isCompact ? 'p-3' : 'p-4 sm:p-5'}`}>
          {showMetrics && (
            <div className={`flex items-center justify-between ${isCompact ? 'mb-2' : 'mb-4'}`}>
              <div className="flex items-center gap-2">
                <div className="bg-blue-500/20 backdrop-blur-sm rounded-lg p-1.5 border border-blue-500/30">
                  <ImageIcon className="w-4 h-4 text-blue-400" />
                </div>
                <div>
                  <h3 className={`font-semibold text-white ${isCompact ? 'text-xs' : 'text-sm sm:text-base'}`}>
                    {title}
                  </h3>
                  {!isCompact && <p className="text-white/40 text-[10px]">AI Object Detection</p>}
                </div>
              </div>
              <div className="flex items-center gap-2 sm:gap-3">
                <div className="flex flex-col items-end gap-0.5">
                  <div className="flex items-center gap-1">
                    <TargetIcon className="w-3 h-3 text-emerald-400" />
                    <span className={`font-semibold text-white ${isCompact ? 'text-xs' : 'text-sm'}`}>
                      {metrics.total_detections ?? 0}
                    </span>
                  </div>
                  {!isCompact && (
                    <div className="flex items-center gap-1">
                      <ClockIcon className="w-3 h-3 text-amber-400/60" />
                      <span className="text-white/50 text-[10px]">{Math.round(metrics.avg_processing_time_ms || 0)}ms</span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {!image ? (
            <div
              onDrop={handleDrop}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
              onClick={() => fileInputRef.current?.click()}
              className={`relative border-2 border-dashed rounded-xl text-center cursor-pointer transition-all duration-200 ease-out group ${
                isDragging ? 'border-blue-400 bg-blue-500/10 scale-[1.02]' : 'border-white/20 hover:border-blue-400/50 hover:bg-white/5'
              } ${isCompact ? 'p-4' : 'p-6 sm:p-8'}`}
            >
              <input
                ref={fileInputRef}
                type="file"
                accept="image/jpeg,image/png,image/webp"
                className="hidden"
                onChange={(e) => { const file = e.target.files?.[0]; if (file) handleImageUpload(file) }}
              />
              <div className="flex flex-col items-center gap-2 sm:gap-3">
                <div className={`rounded-full transition-colors ${
                  isDragging ? 'bg-blue-500/30' : 'bg-blue-500/10 group-hover:bg-blue-500/20'
                } ${isCompact ? 'p-3' : 'p-4'}`}>
                  <UploadIcon className={`${isCompact ? 'w-5 h-5' : 'w-6 h-6 sm:w-8 sm:h-8'} text-blue-400`} />
                </div>
                <div>
                  <p className={`font-medium text-white ${isCompact ? 'text-xs' : 'text-sm sm:text-base'}`}>
                    {isDragging ? 'Drop image here' : 'Drop image or click to upload'}
                  </p>
                  {!isCompact && <p className="text-white/40 text-xs mt-1">JPEG, PNG, WebP (max {maxImageSize / 1024 / 1024}MB)</p>}
                </div>
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              <div className="relative bg-black/40 rounded-xl overflow-hidden border border-white/10">
                <div className="relative">
                  <img src={image} alt="Uploaded" className={`w-full h-auto object-contain ${isCompact ? 'max-h-40' : 'max-h-48 sm:max-h-64 md:max-h-80'}`} />
                  {analyzing && (
                    <div className="absolute inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center">
                      <div className="flex flex-col items-center gap-2">
                        <div className="relative">
                          <div className="w-10 h-10 border-2 border-blue-400/30 rounded-full" />
                          <div className="absolute inset-0 w-10 h-10 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
                        </div>
                        <p className="text-white/80 text-xs font-medium">Analyzing...</p>
                      </div>
                    </div>
                  )}
                  {!analyzing && renderBoundingBoxes()}
                </div>
              </div>

              <div className={`flex items-center gap-2 ${isCompact ? 'flex-wrap' : 'flex-wrap sm:flex-nowrap'}`}>
                <button
                  onClick={() => { setImage(null); setResult(null); setError(null); setImageSize(null) }}
                  className="flex items-center gap-1 px-2.5 py-1.5 bg-white/10 hover:bg-white/20 rounded-lg text-white/80 text-xs font-medium transition-colors"
                >
                  <ClearIcon className="w-3.5 h-3.5" />
                  <span className="hidden sm:inline">Clear</span>
                </button>

                {result && filteredDetections.length > 0 && (
                  <div className="flex items-center gap-1.5 bg-emerald-500/20 backdrop-blur-sm rounded-lg px-2.5 py-1.5 border border-emerald-500/30">
                    <TargetIcon className="w-3.5 h-3.5 text-emerald-400" />
                    <span className="text-emerald-300 text-xs font-semibold">{filteredDetections.length} objects</span>
                    <span className="text-emerald-400/50 text-[10px]">|</span>
                    <span className="text-emerald-300/70 text-[10px]">{result.processing_time_ms}ms</span>
                  </div>
                )}

                <div className="flex-1" />

                <button
                  onClick={analyzeImage}
                  disabled={analyzing}
                  className={`flex items-center gap-1.5 px-3 sm:px-4 py-1.5 rounded-lg text-sm font-semibold transition-all duration-200 ${
                    analyzing
                      ? 'bg-blue-500/50 text-white/60 cursor-not-allowed'
                      : 'bg-blue-500 hover:bg-blue-600 text-white shadow-lg shadow-blue-500/25 hover:shadow-blue-500/40'
                  }`}
                >
                  {analyzing ? (
                    <>
                      <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                      <span>Analyzing</span>
                    </>
                  ) : (
                    <>
                      <RefreshIcon className="w-3.5 h-3.5" />
                      <span>Analyze</span>
                    </>
                  )}
                </button>
              </div>

              {result && filteredDetections.length > 0 && !isCompact && (
                <div className="bg-white/5 backdrop-blur-sm rounded-xl p-3 border border-white/10">
                  <p className="text-white/50 text-[10px] font-medium mb-2 uppercase tracking-wider">Detected Objects</p>
                  <div className="flex flex-wrap gap-1.5">
                    {filteredDetections.map((d, i) => (
                      <span
                        key={i}
                        className="px-2 py-1 rounded-md text-[11px] font-medium transition-colors hover:scale-105"
                        style={{
                          backgroundColor: `${COLORS[i % COLORS.length]}20`,
                          color: COLORS[i % COLORS.length],
                          border: `1px solid ${COLORS[i % COLORS.length]}40`
                        }}
                      >
                        {d.class_name} {Math.round(d.confidence * 100)}%
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {error && (
                <div className="bg-red-500/20 backdrop-blur-sm rounded-lg p-2.5 border border-red-500/30">
                  <p className="text-red-300 text-xs">⚠️ {error}</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    )
  }
)

ImageAnalyzer.displayName = 'ImageAnalyzer'
export default { ImageAnalyzer }
