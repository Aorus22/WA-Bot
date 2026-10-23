import * as React from "react"

import { cn } from "@/lib/utils"

function BoxedList({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="boxed-list"
      className={cn(className)}
      {...props}
    />
  )
}

export { BoxedList }
