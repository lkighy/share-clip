import { Toaster as Sonner, type ToasterProps } from "sonner";

const Toaster = ({ ...props }: ToasterProps) => {
  return <Sonner richColors closeButton position="top-right" {...props} />;
};

export { Toaster };
