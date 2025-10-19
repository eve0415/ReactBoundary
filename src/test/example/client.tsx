"use client";

import type { FC } from "react";

const ClientComponentDefaultExport: FC = () => {
  return <div>This is a client-side component. (Default export)</div>;
};

export const ClientComponentNamedExport: FC = () => {
  return <div>This is a client-side component. (Named export)</div>;
};

export function ClientComponentFunctionExport() {
  return <div>This is a client-side component. (Function export)</div>;
}

export default ClientComponentDefaultExport;
