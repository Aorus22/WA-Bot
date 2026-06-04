import { useState } from "react"
import { ThemeProvider } from "@/components/theme-provider"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Smartphone, Loader2, ShieldCheck } from "lucide-react"
import { QRCodeSVG } from "qrcode.react"

interface LoginPageProps {
	qrCode: string | null
	isConnected: boolean
}

export function LoginPage({ qrCode, isConnected }: LoginPageProps) {
	const [activeTab, setActiveTab] = useState<"qr" | "phone">("qr")

	return (
		<ThemeProvider defaultTheme="system" storageKey="wa-bot-theme">
			<div className="min-h-screen bg-[#f0f2f5] dark:bg-[#111b21] flex flex-col items-center justify-center p-4">
				<div className="fixed top-0 left-0 w-full h-[220px] bg-[#00a884] z-0" />

				<div className="z-10 w-full max-w-[1000px] flex flex-col items-center gap-8">
					<div className="flex items-center gap-3 self-start text-white mb-2">
						<svg viewBox="0 0 24 24" width="36" height="36" fill="currentColor">
							<path d="M12.011 2.25c-5.385 0-9.766 4.381-9.766 9.766 0 1.838.513 3.557 1.403 5.022L2.25 21.75l4.891-1.282c1.417.771 3.033 1.214 4.75 1.214 5.385 0 9.766-4.381 9.766-9.766 0-5.385-4.381-9.766-9.766-9.766zm5.834 13.84c-.242.683-1.192 1.238-1.95 1.314-.541.054-1.246.082-2.008-.184-2.956-1.033-4.869-4.04-5.016-4.237-.148-.197-1.077-1.432-1.077-2.73 0-1.299.683-1.939.927-2.203.243-.264.536-.33.714-.33h.509c.162 0 .378.014.546.417.172.417.587 1.432.637 1.536.051.104.084.225.014.364s-.104.225-.208.347c-.104.122-.219.273-.312.368-.104.104-.213.218-.092.428.122.211.543.896 1.166 1.451.802.714 1.474.936 1.684 1.04.211.104.334.087.458-.056.124-.143.536-.624.68-.839.143-.216.287-.181.484-.11.197.071 1.246.587 1.463.693.216.106.359.159.412.25s.053.541-.189 1.224z" />
						</svg>
						<span className="text-sm font-bold tracking-widest uppercase">WhatsApp Bot</span>
					</div>

					<Card className="w-full border-none shadow-xl bg-white dark:bg-[#222e35] rounded-none md:rounded-sm overflow-hidden min-h-[500px] flex flex-col md:flex-row">
						<div className="flex-1 p-8 md:p-12">
							<div className="space-y-6">
								<div>
									<h1 className="text-3xl font-light text-[#41525d] dark:text-[#e9edef] mb-2">Use WhatsApp on your computer</h1>
									<ol className="space-y-4 mt-8 text-[#667781] dark:text-[#aebac1] text-[15px]">
										<li className="flex gap-4">
											<span className="flex-shrink-0 w-6 h-6 rounded-full border border-current flex items-center justify-center text-xs">1</span>
											<span>Open WhatsApp on your phone</span>
										</li>
										<li className="flex gap-4">
											<span className="flex-shrink-0 w-6 h-6 rounded-full border border-current flex items-center justify-center text-xs">2</span>
											<span>Tap <b>Menu</b> or <b>Settings</b> and select <b>Linked Devices</b></span>
										</li>
										<li className="flex gap-4">
											<span className="flex-shrink-0 w-6 h-6 rounded-full border border-current flex items-center justify-center text-xs">3</span>
											<span>Point your phone to this screen to capture the code</span>
										</li>
									</ol>
								</div>

								<div className="pt-8">
									<Button
										variant="link"
										className="text-[#00a884] p-0 h-auto font-medium hover:no-underline"
										onClick={() => setActiveTab(activeTab === "qr" ? "phone" : "qr")}
									>
										{activeTab === "qr" ? "Link with phone number" : "Link with QR code"}
									</Button>
								</div>

								<div className="pt-12 flex items-center gap-2 text-[#667781] dark:text-[#aebac1] text-xs">
									<ShieldCheck className="h-4 w-4" />
									<span>Your personal messages are end-to-end encrypted</span>
								</div>
							</div>
						</div>

						<div className="w-full md:w-[400px] bg-[#f9f9fa] dark:bg-[#1c272d] flex flex-col items-center justify-center p-8 border-t md:border-t-0 md:border-l border-[#f2f2f2] dark:border-[#2a3942]">
							{activeTab === "qr" ? (
								<div className="relative p-4 bg-white rounded-sm shadow-sm">
									{!isConnected ? (
										<div className="flex flex-col items-center justify-center w-[264px] h-[264px] gap-3">
											<Loader2 className="h-10 w-10 text-[#00a884] animate-spin" />
											<p className="text-sm font-medium text-muted-foreground text-center px-4">Connecting to server...</p>
										</div>
									) : qrCode ? (
										<div className="animate-in fade-in duration-500">
											<QRCodeSVG
												value={qrCode}
												size={264}
												level="M"
												includeMargin={false}
												className="dark:bg-white p-2"
											/>
										</div>
									) : (
										<div className="flex flex-col items-center justify-center w-[264px] h-[264px] gap-3">
											<Loader2 className="h-10 w-10 text-[#00a884] animate-spin" />
											<p className="text-sm font-medium text-muted-foreground">Waiting for QR code...</p>
										</div>
									)}
								</div>
							) : (
								<div className="w-full space-y-6 flex flex-col items-center">
									<div className="w-16 h-16 bg-[#00a884]/10 rounded-full flex items-center justify-center">
										<Smartphone className="h-8 w-8 text-[#00a884]" />
									</div>
									<div className="text-center">
										<h3 className="text-lg font-medium">Link with phone number</h3>
										<p className="text-sm text-muted-foreground mt-1">This feature is coming soon to the bot interface.</p>
									</div>
									<Button
										onClick={() => setActiveTab("qr")}
										variant="outline"
										className="rounded-full border-[#00a884] text-[#00a884] hover:bg-[#00a884]/10"
									>
										Go back to QR code
									</Button>
								</div>
							)}
						</div>
					</Card>
				</div>

				<div className="mt-12 text-[#8696a0] text-sm text-center max-w-[500px]">
					This is a web-based client for your WhatsApp Bot. Please make sure your server is running and connected to the internet.
				</div>
			</div>
		</ThemeProvider>
	)
}
