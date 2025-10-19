import type { FC } from "react";
import ClientComponentDefaultExport, {
  ClientComponentFunctionExport,
  ClientComponentNamedExport,
} from "./client";

const ServerComponent: FC = () => {
  return (
    <div>
      This is a server-side component.
      <ClientComponentDefaultExport />
      <ClientComponentNamedExport />
      <ClientComponentFunctionExport />
    </div>
  );
};

export default ServerComponent;
