import { useEffect, useRef, useCallback } from 'react'
import { animate } from 'animejs'
import { Shield } from 'lucide-react'

interface AnimatedLicenseProps {
  agentId: number
  walletAddress: string
  isActive: boolean
  name?: string | null
  chainId?: number
}

export default function AnimatedLicense({ agentId, walletAddress, isActive, name, chainId }: AnimatedLicenseProps) {
  const cardRef = useRef<HTMLDivElement>(null)
  const sheenRef = useRef<HTMLDivElement>(null)
  const particleContainerRef = useRef<HTMLDivElement>(null)
  const glowRef = useRef<HTMLDivElement>(null)
  const scanlineRef = useRef<HTMLDivElement>(null)

  const shortenAddress = (addr: string) => {
    if (addr.length <= 12) return addr
    return `${addr.slice(0, 6)}...${addr.slice(-4)}`
  }

  const formatAgentId = (id: number) => {
    return `#${String(id).padStart(4, '0')}`
  }

  const networkName = chainId === 8453 ? 'BASE' : chainId === 84532 ? 'BASE SEPOLIA' : `CHAIN ${chainId ?? '?'}`

  // Holographic sheen sweep
  const startSheenAnimation = useCallback(() => {
    if (!sheenRef.current) return
    const loop = () => {
      animate(sheenRef.current!, {
        translateX: ['-120%', '220%'],
        duration: 2400,
        ease: 'inOutQuad',
        onComplete: () => setTimeout(loop, 3000),
      })
    }
    setTimeout(loop, 1000)
  }, [])

  // Floating particles around the card
  const startParticles = useCallback(() => {
    if (!particleContainerRef.current) return
    const container = particleContainerRef.current
    const colors = ['#3b82f6', '#60a5fa', '#8b5cf6', '#06b6d4', '#818cf8']

    const spawnParticle = () => {
      const particle = document.createElement('div')
      const size = 2 + Math.random() * 4
      const color = colors[Math.floor(Math.random() * colors.length)]
      const side = Math.floor(Math.random() * 4)

      let startX: number, startY: number, endX: number, endY: number

      const w = container.offsetWidth
      const h = container.offsetHeight
      switch (side) {
        case 0: // top
          startX = Math.random() * w; startY = -10
          endX = startX + (Math.random() - 0.5) * 80; endY = -60 - Math.random() * 40
          break
        case 1: // right
          startX = w + 10; startY = Math.random() * h
          endX = w + 60 + Math.random() * 40; endY = startY + (Math.random() - 0.5) * 80
          break
        case 2: // bottom
          startX = Math.random() * w; startY = h + 10
          endX = startX + (Math.random() - 0.5) * 80; endY = h + 60 + Math.random() * 40
          break
        default: // left
          startX = -10; startY = Math.random() * h
          endX = -60 - Math.random() * 40; endY = startY + (Math.random() - 0.5) * 80
          break
      }

      particle.style.cssText = `
        position: absolute;
        width: ${size}px;
        height: ${size}px;
        background: ${color};
        border-radius: 50%;
        left: ${startX}px;
        top: ${startY}px;
        pointer-events: none;
        box-shadow: 0 0 ${size * 3}px ${color};
      `
      container.appendChild(particle)

      animate(particle, {
        translateX: [0, endX - startX],
        translateY: [0, endY - startY],
        opacity: [0, 1, 0],
        scale: [0, 1.5, 0],
        duration: 1500 + Math.random() * 1500,
        ease: 'outQuad',
        onComplete: () => particle.remove(),
      })
    }

    const interval = setInterval(spawnParticle, 200)
    return () => clearInterval(interval)
  }, [])

  // Scanline sweep
  const startScanline = useCallback(() => {
    if (!scanlineRef.current) return
    const loop = () => {
      animate(scanlineRef.current!, {
        translateY: ['-100%', '500%'],
        opacity: [0, 0.6, 0],
        duration: 3000,
        ease: 'inOutSine',
        onComplete: () => setTimeout(loop, 4000),
      })
    }
    setTimeout(loop, 2000)
  }, [])

  // Card float
  const startFloat = useCallback(() => {
    if (!cardRef.current) return
    animate(cardRef.current, {
      translateY: [0, -12, 0],
      rotateX: [0, 1, 0],
      rotateY: [0, -1.5, 0],
      duration: 6000,
      ease: 'inOutSine',
      loop: true,
    })
  }, [])

  // Glow pulse
  const startGlow = useCallback(() => {
    if (!glowRef.current) return
    animate(glowRef.current, {
      opacity: [0.4, 0.8, 0.4],
      scale: [0.95, 1.05, 0.95],
      duration: 4000,
      ease: 'inOutSine',
      loop: true,
    })
  }, [])

  // Mouse tilt
  useEffect(() => {
    const card = cardRef.current
    if (!card) return

    const handleMove = (e: MouseEvent) => {
      const rect = card.getBoundingClientRect()
      const centerX = rect.left + rect.width / 2
      const centerY = rect.top + rect.height / 2
      const rotateY = ((e.clientX - centerX) / (rect.width / 2)) * 8
      const rotateX = ((centerY - e.clientY) / (rect.height / 2)) * 5

      card.style.transform = `perspective(1000px) rotateX(${rotateX}deg) rotateY(${rotateY}deg)`
    }

    const handleLeave = () => {
      card.style.transform = ''
    }

    card.addEventListener('mousemove', handleMove)
    card.addEventListener('mouseleave', handleLeave)
    return () => {
      card.removeEventListener('mousemove', handleMove)
      card.removeEventListener('mouseleave', handleLeave)
    }
  }, [])

  useEffect(() => {
    startSheenAnimation()
    startFloat()
    startGlow()
    startScanline()
    const cleanup = startParticles()
    return cleanup
  }, [startSheenAnimation, startFloat, startGlow, startScanline, startParticles])

  return (
    <div className="relative flex items-center justify-center py-8">
      {/* Outer glow */}
      <div
        ref={glowRef}
        className="absolute w-[480px] h-[320px] rounded-3xl pointer-events-none"
        style={{
          background: 'radial-gradient(ellipse at center, rgba(59,130,246,0.25) 0%, rgba(139,92,246,0.15) 40%, transparent 70%)',
          filter: 'blur(40px)',
        }}
      />

      {/* Particle container */}
      <div
        ref={particleContainerRef}
        className="absolute w-[460px] h-[300px] pointer-events-none"
      />

      {/* The license card */}
      <div
        ref={cardRef}
        className="relative w-[420px] h-[260px] rounded-2xl overflow-hidden cursor-default select-none"
        style={{
          perspective: '1000px',
          transformStyle: 'preserve-3d',
        }}
      >
        {/* Card background */}
        <div
          className="absolute inset-0 rounded-2xl"
          style={{
            background: 'linear-gradient(135deg, #0f172a 0%, #1e1b4b 30%, #172554 60%, #0c0a1d 100%)',
            border: '1px solid rgba(99, 102, 241, 0.3)',
          }}
        />

        {/* Circuit pattern overlay */}
        <div
          className="absolute inset-0 opacity-[0.04]"
          style={{
            backgroundImage: `
              linear-gradient(0deg, rgba(255,255,255,0.1) 1px, transparent 1px),
              linear-gradient(90deg, rgba(255,255,255,0.1) 1px, transparent 1px)
            `,
            backgroundSize: '20px 20px',
          }}
        />

        {/* Holographic sheen */}
        <div
          ref={sheenRef}
          className="absolute inset-0 pointer-events-none"
          style={{
            background: 'linear-gradient(105deg, transparent 30%, rgba(99,102,241,0.12) 40%, rgba(139,92,246,0.15) 45%, rgba(6,182,212,0.1) 50%, rgba(59,130,246,0.12) 55%, transparent 65%)',
            width: '100%',
            height: '100%',
            transform: 'translateX(-120%)',
          }}
        />

        {/* Scanline */}
        <div
          ref={scanlineRef}
          className="absolute inset-x-0 h-[2px] pointer-events-none"
          style={{
            background: 'linear-gradient(90deg, transparent 0%, rgba(59,130,246,0.5) 20%, rgba(139,92,246,0.6) 50%, rgba(59,130,246,0.5) 80%, transparent 100%)',
            boxShadow: '0 0 15px rgba(99,102,241,0.4)',
            transform: 'translateY(-100%)',
          }}
        />

        {/* Card content */}
        <div className="relative z-10 h-full flex flex-col justify-between p-6">
          {/* Top row */}
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-blue-500/30">
                <Shield className="w-5 h-5 text-white" />
              </div>
              <div>
                <div className="text-xs font-mono text-blue-400/70 tracking-widest uppercase">EIP-8004</div>
                <div className="text-lg font-bold text-white tracking-wide">STARK LICENSE</div>
              </div>
            </div>
            <div className="text-right">
              <div className="text-[10px] font-mono text-white/30 tracking-wider">NETWORK</div>
              <div className="text-xs font-mono text-blue-400 flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-blue-400 inline-block animate-pulse" />
                {networkName}
              </div>
            </div>
          </div>

          {/* Center - Agent ID */}
          <div className="flex-1 flex flex-col justify-center items-center">
            <div className="text-[10px] font-mono text-white/25 tracking-[0.3em] mb-1">AGENT IDENTITY</div>
            <div className="text-2xl font-mono font-bold text-white/90 tracking-wider license-id-glow">
              {formatAgentId(agentId)}
            </div>
            <div className="text-[10px] font-mono text-white/20 mt-1 tracking-widest">
              {name ? name.toUpperCase() : 'STARKBOT AGENT LICENSE'}
            </div>
          </div>

          {/* Bottom row */}
          <div className="flex items-end justify-between">
            <div>
              <div className="text-[10px] font-mono text-white/25 tracking-wider">OWNER</div>
              <div className="text-xs font-mono text-white/50">{shortenAddress(walletAddress)}</div>
            </div>
            <div className="text-right">
              <div className="text-[10px] font-mono text-white/25 tracking-wider">STATUS</div>
              <div className="flex items-center gap-1.5">
                <span className={`w-1.5 h-1.5 rounded-full ${isActive ? 'bg-green-400' : 'bg-red-400'} animate-pulse`} />
                <span className={`text-xs font-mono ${isActive ? 'text-green-400' : 'text-red-400'}`}>
                  {isActive ? 'ACTIVE' : 'INACTIVE'}
                </span>
              </div>
            </div>
            <div className="text-right">
              <div className="text-[10px] font-mono text-white/25 tracking-wider">FEE</div>
              <div className="text-xs font-mono text-white/50">1,000 STARK</div>
            </div>
          </div>
        </div>

        {/* Corner accents */}
        <div className="absolute top-0 left-0 w-8 h-8 border-t-2 border-l-2 border-blue-500/30 rounded-tl-2xl" />
        <div className="absolute top-0 right-0 w-8 h-8 border-t-2 border-r-2 border-blue-500/30 rounded-tr-2xl" />
        <div className="absolute bottom-0 left-0 w-8 h-8 border-b-2 border-l-2 border-indigo-500/30 rounded-bl-2xl" />
        <div className="absolute bottom-0 right-0 w-8 h-8 border-b-2 border-r-2 border-indigo-500/30 rounded-br-2xl" />
      </div>
    </div>
  )
}
