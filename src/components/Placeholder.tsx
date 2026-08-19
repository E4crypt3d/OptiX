import { Construction } from "lucide-react";
import { Card } from "./ui";

export function Placeholder({ title, description }: { title: string; description: string }) {
  return (
    <div className="space-y-4">
      <header>
        <h1 className="text-xl font-semibold text-slate-100">{title}</h1>
        <p className="text-sm text-slate-500">{description}</p>
      </header>
      <Card>
        <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
          <Construction className="h-8 w-8 text-slate-600" />
          <p className="text-sm text-slate-400">
            This module ships in a later phase of Optix.
          </p>
        </div>
      </Card>
    </div>
  );
}
