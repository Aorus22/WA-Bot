import { useState, useRef, useEffect } from "react"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

interface LazyMediaProps {
  src: string
  alt?: string
  className?: string
  containerClassName?: string
  threshold?: number
  rootMargin?: string
  type?: "image" | "video"
  onClick?: (e: React.MouseEvent) => void
  controls?: boolean
  loading?: "lazy" | "eager"
}

export function LazyMedia({
  src,
  alt = "",
  className,
  containerClassName,
  threshold = 0.01,
  rootMargin = "300px", // Margin lebih besar agar user tidak melihat loading
  type = "image",
  onClick,
  controls = true,
  loading = "lazy",
  ...props
}: LazyMediaProps) {
  const [isVisible, setIsVisible] = useState(false)
  const [isLoaded, setIsLoaded] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting) {
          setIsVisible(true)
          if (containerRef.current) {
            observer.unobserve(containerRef.current)
          }
        }
      },
      { threshold, rootMargin }
    )

    if (containerRef.current) {
      observer.observe(containerRef.current)
    }

    return () => {
      if (containerRef.current) {
        observer.unobserve(containerRef.current)
      }
    }
  }, [threshold, rootMargin, src])

  const handleLoad = () => setIsLoaded(true)

  return (
    <div 
      ref={containerRef} 
      className={cn("relative overflow-hidden w-full h-full min-h-[100px] bg-muted/20", containerClassName)}
      onClick={onClick}
    >
      {(!isLoaded || !isVisible) && (
        <Skeleton className="absolute inset-0 z-0 w-full h-full" />
      )}
      
      {isVisible && (
        type === "image" ? (
          <img
            src={src}
            alt={alt}
            onLoad={handleLoad}
            loading={loading}
            className={cn(
              "transition-opacity duration-700 ease-in-out",
              isLoaded ? "opacity-100" : "opacity-0",
              className
            )}
            {...(props as any)}
          />
        ) : (
          <video
            src={src}
            onLoadedData={handleLoad}
            controls={controls}
            className={cn(
              "transition-opacity duration-700 ease-in-out",
              isLoaded ? "opacity-100" : "opacity-0",
              className
            )}
            {...(props as any)}
          />
        )
      )}
    </div>
  )
}
