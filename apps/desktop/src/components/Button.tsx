import type { ButtonHTMLAttributes, ReactNode } from 'react'

type Props = ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode; variant?: 'primary' | 'secondary' | 'ghost' | 'danger' }

export function Button({ children, variant = 'secondary', className = '', ...props }: Props) {
  return <button className={`button button-${variant} ${className}`} {...props}>{children}</button>
}
