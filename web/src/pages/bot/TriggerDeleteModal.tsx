import { Trash2 } from "lucide-react"
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

interface TriggerDeleteModalProps {
	open: boolean
	onOpenChange: (open: boolean) => void
	onConfirm: () => void
}

export function TriggerDeleteModal({ open, onOpenChange, onConfirm }: TriggerDeleteModalProps) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent className="rounded-[2rem] border-border/40">
				<AlertDialogHeader>
					<div className="flex items-center gap-3 text-destructive mb-2">
						<div className="p-2 rounded-full bg-destructive/10">
							<Trash2 className="h-5 w-5" />
						</div>
						<AlertDialogTitle className="text-xl">Delete Trigger?</AlertDialogTitle>
					</div>
					<AlertDialogDescription>
						This will permanently remove the trigger and its Lua script. This action cannot be undone.
					</AlertDialogDescription>
				</AlertDialogHeader>
				<AlertDialogFooter className="mt-4">
					<AlertDialogCancel className="rounded-xl">Cancel</AlertDialogCancel>
					<AlertDialogAction
						onClick={onConfirm}
						className="rounded-xl bg-destructive text-destructive-foreground hover:bg-destructive/90 font-bold"
					>
						Delete
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	)
}
