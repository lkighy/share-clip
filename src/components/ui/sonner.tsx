import { Toaster as Sonner, type ToasterProps } from "sonner";

const Toaster = ({ ...props }: ToasterProps) => {
  return <Sonner richColors closeButton position="top-right" theme={document.documentElement.classList.contains("dark") ? "dark" : "light"} {...props} />;
};

export { Toaster };
