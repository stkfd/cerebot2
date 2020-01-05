import {ComponentProps, FunctionComponent} from "react";
import "./button.css";

interface Props extends ComponentProps<'button'> {
    outline?: boolean;
}

const Button: FunctionComponent<Props> = (props: Props) => {
    return <>
        <button
            className="btn" {...props}
        >
            {props.children}
        </button>
    </>;
};

export default Button;
