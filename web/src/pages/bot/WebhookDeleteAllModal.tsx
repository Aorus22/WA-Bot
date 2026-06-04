import { AlertTriangle } from "lucide-react"
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from "@/components/ui/alert-dialog"

interface WebhookDeleteAllModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	onConfirm: () => void
}

export function WebhookDeleteAllModal({ open, onOpenChange, onConfirm }: WebhookDeleteAllModalProps) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent className="rounded-[2rem] border-border/40 max-w-md">
				<AlertDialogHeader>
					<div className="flex items-center gap-3 text-destructive mb-2">
						<div className="p-3 rounded-full bg-destructive/10">
							<AlertTriangle className="h-6 w-6" />
						</div>
						<AlertDialogTitle className="text-2xl font-black">Nuke All Webhooks?</AlertDialogTitle>
					</div>
					<AlertDialogDescription className="text-base">
						You are about to delete **EVERY** webhook endpoint in the system. This is a destructive operation and cannot be reversed.
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter className="mt-6 flex sm:justify-between gap-3">
					<AlertDialogCancel className="rounded-xl flex-1 border-border/40">Abort Mission</AlertDialogCancel>
					<AlertDialogAction
						onClick={onConfirm}
						className="rounded-xl flex-[1.5] bg-destructive text-destructive-foreground hover:bg-destructive/90 font-black uppercase tracking-widest shadow-lg shadow-destructive/20"
					>
						Yes, Delete Everything
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	)
}
