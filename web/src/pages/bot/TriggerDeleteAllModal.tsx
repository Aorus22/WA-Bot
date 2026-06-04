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

interface TriggerDeleteAllModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	onConfirm: () => void
}

export function TriggerDeleteAllModal({ open, onOpenChange, onConfirm }: TriggerDeleteAllModalProps) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent className="rounded-[2rem] border-destructive/20 bg-destructive/5 dark:bg-destructive/[0.02]">
				<AlertDialogHeader>
					<div className="flex items-center gap-3 text-destructive mb-2">
						<div className="p-2 rounded-full bg-destructive/20">
							<AlertTriangle className="h-6 w-6" />
						</div>
						<AlertDialogTitle className="text-2xl font-bold">DANGER: Delete All Triggers?</AlertDialogTitle>
					</div>
					<AlertDialogDescription className="text-destructive/80 font-medium">
						WARNING: This will wipe your entire trigger library. All automated behaviors and custom scripts will be lost forever.
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter className="mt-6">
					<AlertDialogCancel className="rounded-xl bg-background">Keep My Triggers</AlertDialogCancel>
					<AlertDialogAction
						onClick={onConfirm}
						className="rounded-xl bg-destructive text-destructive-foreground hover:bg-destructive/90 font-bold shadow-lg shadow-destructive/20"
					>
						Yes, Wipe Everything
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	)
}
