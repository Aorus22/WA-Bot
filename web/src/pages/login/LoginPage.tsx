import { useState, useEffect, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Smartphone, Loader2, ShieldCheck, ArrowLeft, QrCode, RefreshCw } from "lucide-react"
import { QRCodeSVG } from "qrcode.react"
import { useNavigate } from "react-router-dom"
import { api } from "@/lib/api"

export function LoginPage() {
	const [activeTab, setActiveTab] = useState<"qr" | "phone">("qr")
	const [qrCode, setQrCode] = useState<string | null>(null)
	const [loading, setLoading] = useState(true)
	const [error, setError] = useState<string | null>(null)
	const navigate = useNavigate()

	const fetchQrCode = useCallback(async () => {
		try {
			setLoading(true)
			setError(null)
			const data = await api.getQrCode()
			setQrCode(data.code || null)
		} catch {
			setError("Failed to load QR code")
		} finally {
			setLoading(false)
		}
	}, [])

	useEffect(() => {
		fetchQrCode()
		// Poll every 30 seconds (QR codes expire after ~20-30s on WhatsApp)
		const interval = setInterval(fetchQrCode, 30000)
		return () => clearInterval(interval)
	}, [fetchQrCode])

	return (
		<div className="min-h-screen bg-background flex flex-col items-center justify-center p-4">
			<div className="w-full max-w-lg flex flex-col items-center gap-6">

				{/* Header */}
				<div className="text-center space-y-1">
					<div className="inline-flex items-center justify-center w-14 h-14 rounded-2xl bg-primary/10 text-primary mb-2">
						<QrCode className="h-7 w-7" />
					</div>
					<h1 className="text-2xl font-bold tracking-tight text-foreground">Link your device</h1>
					<p className="text-sm text-muted-foreground">Scan the QR code with your WhatsApp to connect</p>
				</div>

				{/* QR / Phone tabs */}
				<div className="flex bg-muted p-1 rounded-xl border border-border/40 gap-1 w-full max-w-xs">
					<button
						onClick={() => setActiveTab("qr")}
						className={`flex-1 flex items-center justify-center gap-2 py-2 px-4 rounded-lg text-sm font-medium transition-all ${
							activeTab === "qr" ? "bg-background text-foreground shadow-sm border border-border/40" : "text-muted-foreground hover:text-foreground"
						}`}
					>
						<QrCode className="h-4 w-4" />
						QR Code
					</button>
					<button
						onClick={() => setActiveTab("phone")}
						className={`flex-1 flex items-center justify-center gap-2 py-2 px-4 rounded-lg text-sm font-medium transition-all ${
							activeTab === "phone" ? "bg-background text-foreground shadow-sm border border-border/40" : "text-muted-foreground hover:text-foreground"
						}`}
					>
						<Smartphone className="h-4 w-4" />
						Phone
					</button>
				</div>

				{/* Main card */}
				<Card className="w-full border-border/40 shadow-sm">
					<CardContent className="p-6">
						{activeTab === "qr" ? (
							<div className="flex flex-col items-center gap-6">
								<div className="p-4 rounded-2xl bg-muted/50 border border-border/40 flex items-center justify-center min-w-[280px] min-h-[280px]">
									{loading ? (
										<div className="flex flex-col items-center gap-3">
											<Loader2 className="h-8 w-8 text-primary animate-spin" />
											<p className="text-sm text-muted-foreground">Loading QR code...</p>
										</div>
									) : error ? (
										<div className="flex flex-col items-center gap-4">
											<p className="text-sm text-muted-foreground">{error}</p>
											<Button variant="outline" size="sm" onClick={fetchQrCode} className="rounded-full gap-1.5">
												<RefreshCw className="h-3.5 w-3.5" />
												Retry
											</Button>
										</div>
									) : qrCode ? (
										<div className="animate-in fade-in duration-500">
											<QRCodeSVG
												value={qrCode}
												size={240}
												level="M"
												includeMargin={false}
												className="bg-white p-3 rounded-xl"
											/>
										</div>
									) : (
										<div className="flex flex-col items-center gap-3">
											<Loader2 className="h-8 w-8 text-muted-foreground/40" />
											<p className="text-sm text-muted-foreground">No QR code available yet</p>
										</div>
									)}
								</div>

								{qrCode && (
									<Button variant="outline" size="sm" onClick={fetchQrCode} className="rounded-full gap-1.5" disabled={loading}>
										<RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`} />
										Refresh QR
									</Button>
								)}

								<div className="text-center space-y-4">
									<ol className="space-y-3 text-sm text-muted-foreground text-left max-w-xs">
										<li className="flex gap-3">
											<span className="flex-shrink-0 w-6 h-6 rounded-full bg-muted border border-border/40 flex items-center justify-center text-xs font-medium text-foreground">1</span>
											<span>Open WhatsApp on your phone</span>
										</li>
										<li className="flex gap-3">
											<span className="flex-shrink-0 w-6 h-6 rounded-full bg-muted border border-border/40 flex items-center justify-center text-xs font-medium text-foreground">2</span>
											<span>Tap <b>Menu</b> or <b>Settings</b> and select <b>Linked Devices</b></span>
										</li>
										<li className="flex gap-3">
											<span className="flex-shrink-0 w-6 h-6 rounded-full bg-muted border border-border/40 flex items-center justify-center text-xs font-medium text-foreground">3</span>
											<span>Point your phone to scan the QR code</span>
										</li>
									</ol>
								</div>
							</div>
						) : (
							<div className="flex flex-col items-center gap-6">
								<div className="w-16 h-16 rounded-2xl bg-primary/10 flex items-center justify-center">
									<Smartphone className="h-8 w-8 text-primary" />
								</div>
								<div className="text-center space-y-1">
									<h3 className="text-lg font-semibold text-foreground">Link with phone number</h3>
									<p className="text-sm text-muted-foreground">This feature is coming soon to the bot interface.</p>
								</div>
								<Button
									onClick={() => setActiveTab("qr")}
									variant="outline"
									size="sm"
									className="rounded-full gap-1.5"
								>
									<ArrowLeft className="h-3.5 w-3.5" />
									Back to QR code
								</Button>
							</div>
						)}
					</CardContent>
				</Card>

				{/* Footer */}
				<div className="flex items-center gap-2 text-xs text-muted-foreground">
					<ShieldCheck className="h-3.5 w-3.5" />
					<span>End-to-end encrypted</span>
				</div>

				<Button
					variant="ghost"
					size="sm"
					className="text-muted-foreground text-xs"
					onClick={() => navigate(-1)}
				>
					← Back
				</Button>
			</div>
		</div>
	)
}
