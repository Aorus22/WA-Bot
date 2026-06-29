import { useLocation, useNavigate } from "react-router-dom"
import { MessageSquare, Bot, FileText, Clock, Globe, Settings } from "lucide-react"
import { cn } from "@/lib/utils"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export function NavigationSidebar() {
	const location = useLocation()
	const navigate = useNavigate()

	const isActive = (path: string) => location.pathname.startsWith(path)

	return (
		<div className="w-[68px] h-full flex flex-col items-center py-6 bg-background border-r border-border/40 flex-shrink-0 z-50">
			<div className="flex flex-col gap-4 flex-1">
				<NavButton
					icon={<MessageSquare className="h-5 w-5" />}
					label="Chats"
					isActive={isActive("/chat")}
					onClick={() => navigate("/chat")}
				/>
				<NavButton
					icon={<Bot className="h-5 w-5" />}
					label="Triggers"
					isActive={isActive("/triggers")}
					onClick={() => navigate("/triggers")}
				/>
				<NavButton
					icon={<Clock className="h-5 w-5" />}
					label="Cron Jobs"
					isActive={isActive("/cron")}
					onClick={() => navigate("/cron")}
				/>
				<NavButton
					icon={<Globe className="h-5 w-5" />}
					label="Webhooks"
					isActive={isActive("/webhooks")}
					onClick={() => navigate("/webhooks")}
				/>
				<NavButton
					icon={<FileText className="h-5 w-5" />}
					label="Documentation"
					isActive={isActive("/documentation")}
					onClick={() => navigate("/documentation")}
				/>
			</div>

			<div className="mt-auto pb-4">
				<NavButton
					icon={<Settings className="h-5 w-5" />}
					label="Settings"
					isActive={isActive("/settings")}
					onClick={() => navigate("/settings")}
				/>
			</div>
		</div>
	)
}

function NavButton({ icon, label, isActive, onClick }: { icon: React.ReactNode; label: string; isActive: boolean; onClick: () => void }) {
	return (
		<Tooltip delayDuration={0}>
			<TooltipTrigger asChild>
				<button
					onClick={onClick}
					className={cn(
						"p-3.5 rounded-2xl transition-all duration-200 group relative",
						isActive
							? "bg-primary text-primary-foreground shadow-lg shadow-primary/20 scale-105"
							: "text-muted-foreground hover:bg-muted hover:text-foreground"
					)}
				>
					{icon}
					{isActive && (
						<div className="absolute -left-[14px] top-1/2 -translate-y-1/2 w-1 h-6 bg-primary rounded-r-full" />
					)}
				</button>
			</TooltipTrigger>
			<TooltipContent side="right" sideOffset={10} className="font-medium">
				<p>{label}</p>
			</TooltipContent>
		</Tooltip>
	)
}
