import type { FC } from "react";
import ClientComponentDefaultExport, {
  ClientComponentNamedExport,
} from "./client";

const ServerComponent: FC = () => {
  return (
    <div>
      This is a server-side component.
      <ClientComponentDefaultExport />
      <ClientComponentNamedExport />
    </div>
  );
};

export default ServerComponent;
