import { AlertDialog } from "radix-ui";
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
      <AlertDialog.Root>
        <AlertDialog.Trigger>Open Alert Dialog</AlertDialog.Trigger>
        <AlertDialog.Content>
          <AlertDialog.Title>Alert</AlertDialog.Title>
          <AlertDialog.Description>
            This is an alert dialog.
          </AlertDialog.Description>
          <AlertDialog.Action>OK</AlertDialog.Action>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </div>
  );
};

export default ServerComponent;
