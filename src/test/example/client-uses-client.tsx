"use client";

import { ClientComponentNamedExport } from "./client";
import type { FC } from "react";

// This client component uses another client component
// Should NOT show orange decoration on <ClientComponentNamedExport />
// because we're already in client context
export const ClientUsesClientNamedFunction: FC = () => {
  return (
    <div>
      <ClientComponentNamedExport />
    </div>
  );
};

export default function ClientUsesClientDefaultFunction() {
  return <div>This is a client-side component. (Default export)</div>;
}
