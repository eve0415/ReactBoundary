import type { FC } from "react";
import ClientComponent from "./client";

const ServerComponent: FC = () => {
  return (
    <div>
      This is a server-side component.
      <ClientComponent />
    </div>
  );
};

export default ServerComponent;
